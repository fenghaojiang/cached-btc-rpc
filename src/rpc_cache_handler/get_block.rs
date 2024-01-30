use crate::rpc_cache_handler::common::extract_verbose_option;
use crate::rpc_cache_handler::RpcCacheHandler;
use anyhow::{Context, Ok, Result};
use serde_json::Value;

#[derive(Default, Clone)]
pub struct GetBlock;

impl RpcCacheHandler for GetBlock {
    fn method_name(&self) -> &'static str {
        "getblock"
    }

    fn extract_cache_key(&self, params: &Value) -> Result<Option<String>> {
        let params = params
            .as_array()
            .context("params not found or not an array")?;
        let block_hash = params[0].as_str().context("params[0] not a string")?;

        if params.len() == 1 {
            return Ok(Some(format!("{}:{}", block_hash, 1)));
        }

        let verbose = extract_verbose_option(&params[1])?;
        

        Ok(Some(format!("{}:{}", block_hash, verbose)))
    }
}
