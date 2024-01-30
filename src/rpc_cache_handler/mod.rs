use crate::rpc_cache_handler::get_block::GetBlock;
use crate::rpc_cache_handler::get_block_hash::GetBlockHash;
use anyhow::Result;
use serde_json::Value;

mod common;
mod get_block;
mod get_block_hash;

pub trait RpcCacheHandler: Send + Sync {
    fn method_name(&self) -> &'static str;

    fn extract_cache_key(&self, params: &Value) -> Result<Option<String>>;

    fn extract_cache_value(&self, result: &Value) -> Result<(bool, String)> {
        Ok((!result.is_null(), serde_json::to_string(result)?))
    }
}

pub type RpcCacheHandlerFactory = fn() -> Box<dyn RpcCacheHandler>;

pub fn all_factories() -> Vec<RpcCacheHandlerFactory> {
    vec![|| Box::new(GetBlock) as Box<dyn RpcCacheHandler>, || {
        Box::new(GetBlockHash) as Box<dyn RpcCacheHandler>
    }]
}
