# Spectre-miner
[![Build status](https://github.com/tmrlvi/spectre-miner/workflows/ci/badge.svg)](https://github.com/tmrlvi/spectre-miner/actions)
[![Latest version](https://img.shields.io/crates/v/spectre-miner.svg)](https://crates.io/crates/spectre-miner)
![License](https://img.shields.io/crates/l/spectre-miner.svg)
[![dependency status](https://deps.rs/repo/github/tmrlvi/spectre-miner/status.svg)](https://deps.rs/repo/github/tmrlvi/spectre-miner)

--mining-address spectre:x -t 32 -s stratum+tcp://spectre.cedric-crispin.com:4364

## Installation

### From Sources
Installing via `cargo install` is not supported for the latest version.

The regular version is still available at
```sh
cargo install spectre-miner
```

### From Git Sources

If you are looking to build from the repository (for debug / extension), note that the plugins are additional
packages in the workspace. To compile a specific package, you run the following command or any subset of it

```sh
git clone git@github.com:tmrlvi/spectre-miner.git
cd spectre-miner
cargo build --release
```

### From Binaries
The [release page](https://github.com/tmrlvi/spectre-miner/releases) includes precompiled binaries for Linux, and Windows.


# Usage
To start mining, you need to run [spectred](https://github.com/spectrenet/spectred) and have an address to send the rewards to.
Here is a guidance on how to run a full node and how to generate addresses: https://github.com/spectrenet/docs/blob/main/Getting%20Started/Full%20Node%20Installation.md

Help:
```
spectre-miner 
A Spectre high performance CPU miner

USAGE:
    spectre-miner [OPTIONS] --mining-address <MINING_ADDRESS>

OPTIONS:
    -a, --mining-address <MINING_ADDRESS>                  The Spectre address for the miner reward
    -d, --debug                                            Enable debug logging level
        --devfund-percent <DEVFUND_PERCENT>                The percentage of blocks to send to the devfund [default: 0]
    -h, --help                                             Print help information
        --mine-when-not-synced                             Mine even when spectred says it is not synced
        --nonce-gen <NONCE_GEN>                            The random method used to generate nonces. Options: (i) xoshiro (ii) lean [default: lean]
    -p, --port <PORT>                                      Spectred port [default: Mainnet = 18110, Testnet = 18210]
    -s, --spectred-address <SPECTRED_ADDRESS>              The IP of the spectred instance [default: 127.0.0.1]
    -t, --threads <NUM_THREADS>                            Amount of CPU miner threads to launch [default: 0]
        --testnet                                          Use testnet instead of mainnet [default: false]
```

To start mining, you just need to run the following:

`./spectre-miner --mining-address spectre:XXXXX`

# Devfund

The devfund is a fund managed by the Spectre community in order to fund Spectre development <br>
A miner that wants to mine higher percentage into the dev-fund can pass the following flags: <br>
`--devfund-precent=XX.YY` to mine only XX.YY% of the blocks into the devfund.

**This version automatically sets the devfund donation to the community designated address. 
Due to community decision, the minimum amount in the precompiled binaries is 2%**

# Donation Addresses

**Elichai**: `kaspa:qzvqtx5gkvl3tc54up6r8pk5mhuft9rtr0lvn624w9mtv4eqm9rvc9zfdmmpu`

**HauntedCook**: `kaspa:qz4jdyu04hv4hpyy00pl6trzw4gllnhnwy62xattejv2vaj5r0p5quvns058f`
