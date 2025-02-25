use crate::proto::{
    spectred_message::Payload, GetBlockTemplateRequestMessage, GetInfoRequestMessage, NotifyBlockAddedRequestMessage,
    NotifyNewBlockTemplateRequestMessage, RpcBlock, SpectredMessage, SubmitBlockRequestMessage,
};
use crate::{
    pow::{self, HeaderHasher},
    Hash,
};

impl SpectredMessage {
    #[inline(always)]
    pub fn get_info_request() -> Self {
        SpectredMessage { payload: Some(Payload::GetInfoRequest(GetInfoRequestMessage {})) }
    }
    #[inline(always)]
    pub fn notify_block_added() -> Self {
        SpectredMessage { payload: Some(Payload::NotifyBlockAddedRequest(NotifyBlockAddedRequestMessage {})) }
    }

    #[inline(always)]
    pub fn submit_block(block: RpcBlock) -> Self {
        SpectredMessage {
            payload: Some(Payload::SubmitBlockRequest(SubmitBlockRequestMessage {
                block: Some(block),
                allow_non_daa_blocks: false,
            })),
        }
    }
}

impl From<GetInfoRequestMessage> for SpectredMessage {
    fn from(a: GetInfoRequestMessage) -> Self {
        SpectredMessage { payload: Some(Payload::GetInfoRequest(a)) }
    }
}
impl From<NotifyBlockAddedRequestMessage> for SpectredMessage {
    fn from(a: NotifyBlockAddedRequestMessage) -> Self {
        SpectredMessage { payload: Some(Payload::NotifyBlockAddedRequest(a)) }
    }
}

impl From<GetBlockTemplateRequestMessage> for SpectredMessage {
    fn from(a: GetBlockTemplateRequestMessage) -> Self {
        SpectredMessage { payload: Some(Payload::GetBlockTemplateRequest(a)) }
    }
}

impl From<NotifyNewBlockTemplateRequestMessage> for SpectredMessage {
    fn from(a: NotifyNewBlockTemplateRequestMessage) -> Self {
        SpectredMessage { payload: Some(Payload::NotifyNewBlockTemplateRequest(a)) }
    }
}

impl RpcBlock {
    #[inline(always)]
    pub fn block_hash(&self) -> Option<Hash> {
        let mut hasher = HeaderHasher::new();
        pow::serialize_header(&mut hasher, self.header.as_ref()?, false);
        Some(hasher.finalize())
    }
}
