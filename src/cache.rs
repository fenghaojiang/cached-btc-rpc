use crate::rpc_cache_handler::RpcCacheHandler;
use serde_json::Value;

pub(crate) struct CacheEntry {
    pub handler: Box<dyn RpcCacheHandler>,
}

impl CacheEntry {
    pub fn new(handler: Box<dyn RpcCacheHandler>) -> Self {
        Self { handler }
    }
}

pub(crate) enum ResultOrError {
    Error(Value),
    Result(Value),
}

pub(crate) enum CacheStatus {
    NotAvailable,
    Cached(String, Value),
    Missed(String),
}
