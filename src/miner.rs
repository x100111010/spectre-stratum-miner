use std::collections::HashMap;
use std::num::Wrapping;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use crate::{pow, watch, Error};
use log::{error, info, warn};
use rand::{rng, RngCore};
use tokio::sync::mpsc::Sender;
use tokio::task::{self, JoinHandle};

use crate::pow::BlockSeed;

type MinerHandler = std::thread::JoinHandle<Result<(), Error>>;

#[cfg(any(target_os = "linux", target_os = "macos"))]
extern "C" fn signal_panic(_signal: nix::libc::c_int) {
    panic!("Forced shutdown");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn register_freeze_handler() {
    let handler = nix::sys::signal::SigHandler::Handler(signal_panic);
    unsafe {
        nix::sys::signal::signal(nix::sys::signal::Signal::SIGUSR1, handler).unwrap();
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn trigger_freeze_handler(kill_switch: Arc<AtomicBool>, handle: &MinerHandler) -> std::thread::JoinHandle<()> {
    use std::os::unix::thread::JoinHandleExt;
    let pthread_handle = handle.as_pthread_t();
    std::thread::spawn(move || {
        sleep(Duration::from_millis(1000));
        if kill_switch.load(Ordering::SeqCst) {
            match nix::sys::pthread::pthread_kill(pthread_handle, nix::sys::signal::Signal::SIGUSR1) {
                Ok(()) => {
                    info!("Thread killed successfully")
                }
                Err(e) => {
                    info!("Error: {:?}", e)
                }
            }
        }
    })
}

#[cfg(target_os = "windows")]
struct RawHandle(*mut std::ffi::c_void);

#[cfg(target_os = "windows")]
unsafe impl Send for RawHandle {}

#[cfg(target_os = "windows")]
fn register_freeze_handler() {}

#[cfg(target_os = "windows")]
fn trigger_freeze_handler(kill_switch: Arc<AtomicBool>, handle: &MinerHandler) -> std::thread::JoinHandle<()> {
    use std::os::windows::io::AsRawHandle;
    let raw_handle = RawHandle(handle.as_raw_handle());

    std::thread::spawn(move || unsafe {
        let ensure_full_move = raw_handle;
        sleep(Duration::from_millis(1000));
        if kill_switch.load(Ordering::SeqCst) {
            kernel32::TerminateThread(ensure_full_move.0, 0);
        }
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn trigger_freeze_handler(kill_switch: Arc<AtomicBool>, handle: &MinerHandler) {
    warn!("Freeze handler is not implemented. Frozen threads are ignored");
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn register_freeze_handler() {
    warn!("Freeze handler is not implemented. Frozen threads are ignored");
}

#[derive(Clone)]
enum WorkerCommand {
    Job(Box<pow::State>),
    Close,
}

#[allow(dead_code)]
pub struct MinerManager {
    handles: Vec<MinerHandler>,
    block_channel: watch::Sender<Option<WorkerCommand>>,
    send_channel: Sender<BlockSeed>,
    logger_handle: JoinHandle<()>,
    is_synced: bool,
    hashes_tried: Arc<AtomicU64>,
    hashes_by_worker: Arc<Mutex<HashMap<String, Arc<AtomicU64>>>>,
    current_state_id: AtomicUsize,
}

impl Drop for MinerManager {
    fn drop(&mut self) {
        info!("Closing miner");
        self.logger_handle.abort();
        match self.block_channel.send(Some(WorkerCommand::Close)) {
            Ok(_) => {}
            Err(_) => warn!("All workers are already dead"),
        }
        let kill_switch = Arc::new(AtomicBool::new(true));
        while let Some(handle) = self.handles.pop() {
            trigger_freeze_handler(kill_switch.clone(), &handle);
            match handle.join() {
                Ok(res) => match res {
                    Ok(()) => {}
                    Err(e) => error!("Error when closing Worker: {}", e),
                },
                Err(_) => error!("Worker failed to close gracefully"),
            };
            kill_switch.fetch_and(false, Ordering::SeqCst);
        }
    }
}

pub fn get_num_cpus(n_cpus: Option<u16>) -> u16 {
    n_cpus.unwrap_or_else(|| {
        num_cpus::get_physical().try_into().expect("Doesn't make sense to have more than 65,536 CPU cores")
    })
}

const LOG_RATE: Duration = Duration::from_secs(10);

impl MinerManager {
    pub fn new(send_channel: Sender<BlockSeed>, n_cpus: Option<u16>) -> Self {
        register_freeze_handler();
        let hashes_tried = Arc::new(AtomicU64::new(0));
        let hashes_by_worker = Arc::new(Mutex::new(HashMap::<String, Arc<AtomicU64>>::new()));
        let (send, recv) = watch::channel(None);
        let handles = Self::launch_cpu_threads(send_channel.clone(), Arc::clone(&hashes_tried), recv.clone(), n_cpus)
            .collect::<Vec<MinerHandler>>();

        Self {
            handles,
            block_channel: send,
            send_channel,
            logger_handle: task::spawn(Self::log_hashrate(Arc::clone(&hashes_tried), hashes_by_worker.clone())),
            is_synced: true,
            hashes_tried,
            current_state_id: AtomicUsize::new(0),
            hashes_by_worker,
        }
    }

    fn launch_cpu_threads(
        send_channel: Sender<BlockSeed>,
        hashes_tried: Arc<AtomicU64>,
        work_channel: watch::Receiver<Option<WorkerCommand>>,
        n_cpus: Option<u16>,
    ) -> impl Iterator<Item = MinerHandler> {
        let n_cpus = get_num_cpus(n_cpus);
        info!("launching: {} cpu miners", n_cpus);
        (0..n_cpus)
            .map(move |_| Self::launch_cpu_miner(send_channel.clone(), work_channel.clone(), Arc::clone(&hashes_tried)))
    }

    pub async fn process_block(&mut self, block: Option<BlockSeed>) -> Result<(), Error> {
        let state = match block {
            Some(b) => {
                self.is_synced = true;
                let id = self.current_state_id.fetch_add(1, Ordering::SeqCst);
                Some(WorkerCommand::Job(Box::new(pow::State::new(id, b)?)))
            }
            None => {
                if !self.is_synced {
                    return Ok(());
                }
                self.is_synced = false;
                warn!("Spectred is not synced, skipping current template");
                None
            }
        };

        self.block_channel.send(state).map_err(|_e| "Failed sending block to threads")?;
        Ok(())
    }

    #[allow(unreachable_code)]
    fn launch_cpu_miner(
        send_channel: Sender<BlockSeed>,
        mut block_channel: watch::Receiver<Option<WorkerCommand>>,
        hashes_tried: Arc<AtomicU64>,
    ) -> MinerHandler {
        let mut nonce = Wrapping(rng().next_u64());
        let mut mask = Wrapping(0);
        let mut fixed = Wrapping(0);
        std::thread::spawn(move || {
            (|| {
                let mut state = None;

                loop {
                    if state.is_none() {
                        state = match block_channel.wait_for_change() {
                            Ok(cmd) => match cmd {
                                Some(WorkerCommand::Job(s)) => Some(s),
                                Some(WorkerCommand::Close) => {
                                    return Ok(());
                                }
                                None => None,
                            },
                            Err(e) => {
                                info!("CPU thread crashed: {}", e.to_string());
                                return Ok(());
                            }
                        };
                        if let Some(s) = &state {
                            mask = Wrapping(s.nonce_mask);
                            fixed = Wrapping(s.nonce_fixed);
                        }
                    }
                    let state_ref = match state.as_mut() {
                        Some(s) => s,
                        None => continue,
                    };
                    nonce = (nonce & mask) | fixed;

                    if let Some(block_seed) = state_ref.generate_block_if_pow(nonce.0) {
                        match send_channel.blocking_send(block_seed.clone()) {
                            Ok(()) => block_seed.report_block(),
                            Err(e) => error!("Failed submitting block: ({})", e.to_string()),
                        };
                        if let BlockSeed::FullBlock(_) = block_seed {
                            state = None;
                        }
                    }
                    nonce += Wrapping(1);
                    hashes_tried.fetch_add(1, Ordering::AcqRel);
                }
                Ok(())
            })()
            .inspect_err(|e: &Error| {
                error!("CPU thread crashed: {}", e.to_string());
            })
        })
    }

    async fn log_hashrate(hashes_tried: Arc<AtomicU64>, hashes_by_worker: Arc<Mutex<HashMap<String, Arc<AtomicU64>>>>) {
        loop {
            tokio::time::sleep(LOG_RATE).await;
            let hashes = hashes_tried.swap(0, Ordering::AcqRel);
            let rate = (hashes as f64) / LOG_RATE.as_secs_f64();
            if hashes == 0 {
                warn!("Workers stalled or crashed. Consider reducing workload and check that your node is synced");
            } else {
                let (rate, suffix) = Self::hash_suffix(rate);
                info!("Current hashrate is {:.2} {}", rate, suffix);
            }
            for (device, rate) in &*hashes_by_worker.lock().unwrap() {
                let hashes = rate.swap(0, Ordering::AcqRel);
                let rate = (hashes as f64) / LOG_RATE.as_secs_f64();
                if hashes == 0 {
                    warn!("Device {}: 0 hash/s", device);
                } else {
                    let (rate, suffix) = Self::hash_suffix(rate);
                    info!("Device {}: {:.2} {}", device, rate, suffix);
                }
            }
        }
    }

    #[inline]
    fn hash_suffix(n: f64) -> (f64, &'static str) {
        match n {
            n if n < 1_000.0 => (n, "hash/s"),
            n if n < 1_000_000.0 => (n / 1_000.0, "Khash/s"),
            n if n < 1_000_000_000.0 => (n / 1_000_000.0, "Mhash/s"),
            n if n < 1_000_000_000_000.0 => (n / 1_000_000_000.0, "Ghash/s"),
            n if n < 1_000_000_000_000_000.0 => (n / 1_000_000_000_000.0, "Thash/s"),
            _ => (n, "hash/s"),
        }
    }
}

#[cfg(all(test, feature = "bench"))]
mod benches {
    extern crate test;

    use self::test::{black_box, Bencher};
    use crate::pow::State;
    use crate::proto::{RpcBlock, RpcBlockHeader};
    use rand::{rng, RngCore};

    #[bench]
    pub fn bench_mining(bh: &mut Bencher) {
        let mut state = State::new(
            0,
            RpcBlock {
                header: Some(RpcBlockHeader {
                    version: 1,
                    parents: vec![],
                    hash_merkle_root: "23618af45051560529440541e7dc56be27676d278b1e00324b048d410a19d764".to_string(),
                    accepted_id_merkle_root: "947d1a10378d6478b6957a0ed71866812dee33684968031b1cace4908c149d94"
                        .to_string(),
                    utxo_commitment: "ec5e8fc0bc0c637004cee262cef12e7cf6d9cd7772513dbd466176a07ab7c4f4".to_string(),
                    timestamp: 654654353,
                    bits: 0x1e7fffff,
                    nonce: 0,
                    daa_score: 654456,
                    blue_work: "d8e28a03234786".to_string(),
                    pruning_point: "be4c415d378f9113fabd3c09fcc84ddb6a00f900c87cb6a1186993ddc3014e2d".to_string(),
                    blue_score: 1164419,
                }),
                transactions: vec![],
                verbose_data: None,
            },
        )
        .unwrap();
        nonce = rng().next_u64();
        bh.iter(|| {
            for _ in 0..100 {
                black_box(state.check_pow(nonce));
                nonce += 1;
            }
        });
    }
}
