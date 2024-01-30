use crate::rpc_cache_handler::RpcCacheHandler;
use anyhow::Context;
use serde_json::Value;

#[derive(Default, Clone)]
pub struct GetBlockHash;

impl RpcCacheHandler for GetBlockHash {
    fn method_name(&self) -> &'static str {
        "getblockhash"
    }

    fn extract_cache_key(&self, params: &Value) -> anyhow::Result<Option<String>> {
        let params = params
            .as_array()
            .context("params not found or not an array")?;
        let block_number = params[0].as_u64().context("params[0] not a number")?;

        Ok(Some(format!("{}", block_number)))
    }
}
