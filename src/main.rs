use crate::cache::{CacheEntry, CacheStatus, ResultOrError};
use crate::rpc_cache_handler::RpcCacheHandler;
use actix_web::{error, web, App, Error, HttpResponse, HttpServer};
use anyhow::{Context, Result};
use clap::Parser;
use env_logger::Env;
use redis::Commands;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::collections::HashMap;

mod cache;
mod cli;
mod rpc_cache_handler;

struct AppState {
    chains: HashMap<String, ChainState>,
    http_client: Client,
    redis: r2d2::Pool<redis::Client>,
    redis_expired_seconds: u64, 
}

struct ChainState {
    rpc_url: Url,
    cache_entries: HashMap<String, CacheEntry>,
}

impl ChainState {
    fn new(rpc_url: Url) -> Self {
        Self {
            rpc_url,
            cache_entries: Default::default(),
        }
    }
}

async fn request_rpc(client: &Client, rpc_url: Url, body: &Value) -> Result<Value> {
    let result = client
        .post(rpc_url)
        .json(body)
        .send()
        .await?
        .json::<Value>()
        .await?;

    Ok(result)
}

fn read_cache(
    redis_conn: &mut r2d2::PooledConnection<redis::Client>,
    handler: &dyn RpcCacheHandler,
    method: &str,
    params: &Value,
) -> Result<CacheStatus> {
    let cache_key = handler
        .extract_cache_key(params)
        .context("failed to extract cache key")?;
    let cache_key = match cache_key {
        Some(cache_key) => format!("{method}:{cache_key}"),
        None => return Ok(CacheStatus::NotAvailable),
    };

    let value: Option<String> = redis_conn.get(&cache_key).unwrap();

    Ok(if let Some(value) = value {
        let cache_value =
            serde_json::from_str::<Value>(&value).context("failed to deserialize cache value")?;
        CacheStatus::Cached(cache_key, cache_value)
    } else {
        CacheStatus::Missed(cache_key)
    })
}

#[actix_web::post("/{chain}")]
async fn rpc_call(
    path: web::Path<(String,)>,
    data: web::Data<AppState>,
    body: web::Json<Value>,
) -> Result<HttpResponse, Error> {
    let (chain,) = path.into_inner();

    if chain != "bitcoin".to_string() {
        tracing::error!("{}", chain);
        return Err(error::ErrorBadRequest("not supported chain"));
    }

    let chain_state = data
        .chains
        .get(&chain.to_uppercase())
        .ok_or_else(|| error::ErrorNotFound("endpoint not supported"))?;

    let requests = match body {
        web::Json(Value::Array(requests)) => requests,
        web::Json(Value::Object(obj)) => vec![Value::Object(obj)],
        _ => return Err(error::ErrorBadRequest("invalid request body")),
    };

    let mut request_result = HashMap::new();
    let mut uncached_requests = HashMap::new();
    let mut ids_in_original_order = vec![];

    for mut request in requests {
        let id = match request["id"].take() {
            Value::String(n) => {
                let number : u64 = n.parse().expect("can not convert to u64");
                number 
            }
            _ => return Err(error::ErrorBadRequest("invalid id")),
        };

        let method = match request["method"].take() {
            Value::String(s) => s,
            _ => return Err(error::ErrorBadRequest("invalid method")),
        };

        let params = request["params"].take();

        ids_in_original_order.push(id.clone());

        let cache_entry = match chain_state.cache_entries.get(&method) {
            Some(cache_entry) => cache_entry,
            None => {
                uncached_requests.insert(id.clone(), (method, params, None));
                continue;
            }
        };

        let mut redis_conn = data.redis.get().map_err(|err| {
            tracing::error!("fail to get redis connection because: {}", err);
            error::ErrorInternalServerError("fail to get redis connection")
        })?;

        let result = read_cache(
            &mut redis_conn,
            cache_entry.handler.as_ref(),
            &method,
            &params,
        );

        match result {
            Err(err) => {
                tracing::error!("fail to read cache because: {}", err);
                uncached_requests.insert(id, (method, params, None));
            }
            Ok(CacheStatus::NotAvailable) => {
                tracing::info!("cache not available for method {}", method);
                uncached_requests.insert(id, (method, params, None));
            }
            Ok(CacheStatus::Cached(cache_key, value)) => {
                tracing::info!("cache hit for method {} with key {}", method, cache_key);
                request_result.insert(id, ResultOrError::Result(value));
            }
            Ok(CacheStatus::Missed(cache_key)) => {
                tracing::info!("cache missed for method {} with key {}", method, cache_key);
                uncached_requests.insert(id, (method, params, Some(cache_key)));
            }
        }
    }

    if !uncached_requests.is_empty() {
        let request_body = Value::Array(
            uncached_requests
                .iter()
                .map(|(id, (method, params, _))| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id.clone(),
                        "method": method.to_string(),
                        "params": params.clone(),
                    })
                })
                .collect::<Vec<Value>>(),
        );

        let rpc_result = request_rpc(
            &data.http_client,
            chain_state.rpc_url.clone(),
            &request_body,
        ).await.map_err(|err| {
            tracing::error!("failed to make rpc request, err: {}", err);
            return error::ErrorInternalServerError(format!("failed to make rpc request, err: {}", err));
        })?;


        let result_values = match rpc_result {
            Value::Array(v) => v,
            _ => {
                tracing::error!(
                    "array is expected but we got invalid rpc response: {},",
                    rpc_result.to_string()
                );
                return Err(error::ErrorInternalServerError("invalid rpc response"));
            }
        };

        let mut redis_conn = data.redis.get().map_err(|err| {
            tracing::error!("fail to get redis connection because: {}", err);
            error::ErrorInternalServerError("fail to get redis connection")
        })?;

        for mut response in result_values {
            let id_str = response["id"].to_string();
            let id: u64 = id_str.parse().expect("not a valid id");

            let (method, _params, cache_key) = uncached_requests.get(&id).unwrap();

            let error = response["error"].take();
            if !error.is_null() {
                request_result.insert(id, ResultOrError::Error(error.clone()));
                continue;
            }

            let result = response["result"].take();
            request_result.insert(id, ResultOrError::Result(result.clone()));

            let cache_key = match cache_key {
                Some(cache_key) => cache_key.clone(),
                None => continue,
            };

            let cache_entry = chain_state.cache_entries.get(method).unwrap();

            let (can_cache, extracted_value) = cache_entry
                .handler
                .extract_cache_value(&result)
                .expect("failed to extract cache value");

            if can_cache {
                let value = extracted_value.as_str();
                let _ = redis_conn.set_ex::<_, _, String>(cache_key.clone(), value, data.redis_expired_seconds);
            }
        }
    }

    let response = ids_in_original_order
        .iter()
        .map(|id| {
            let result = request_result
                .get_mut(id)
                .unwrap_or_else(|| panic!("result for id {} not found", id));

            match result {
                ResultOrError::Error(error) => {
                    tracing::error!("{:?}", error.to_string());
                    json!({ "jsonrpc": "2.0", "id": id, "error": error.take() })
                }
                ResultOrError::Result(result) => {
                    json!({ "jsonrpc": "2.0", "id": id, "result": result.take() })
                }
            }
        })
        .collect::<Vec<Value>>();

    Ok(HttpResponse::Ok().json(if response.len() == 1 {
        response[0].clone()
    } else {
        Value::Array(response)
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let arg = cli::Cli::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let redis_client = redis::Client::open(arg.redis_url).expect("failed to create redis client");
    let redis_conn_pool = r2d2::Pool::builder()
        .max_size(300)
        .build(redis_client)
        .expect("failed to create redis connection pool");

    let mut app_state = AppState {
        chains: Default::default(),
        http_client: reqwest::Client::new(),
        redis: redis_conn_pool,
        redis_expired_seconds: arg.expired_second,
    };

    let handler_factories = rpc_cache_handler::all_factories();
    tracing::info!("Provisioning cache tables");

    for (name, rpc_url) in arg.endpoints.iter() {
        tracing::info!("Adding endpoint {} linked to {}", name, rpc_url);
        let mut chain_state = ChainState::new(rpc_url.clone());

        for factory in &handler_factories {
            let handler = factory();
            chain_state
                .cache_entries
                .insert(handler.method_name().to_string(), CacheEntry::new(handler));
        }

        app_state.chains.insert(name.to_string(), chain_state);
    }

    let app_state = web::Data::new(app_state);

    tracing::info!("Server listening on {}:{}", arg.bind, arg.port);

    {
        let app_state = app_state.clone();
        HttpServer::new(move || App::new().service(rpc_call).app_data(app_state.clone()))
            .bind((arg.bind, arg.port))?
            .run()
            .await?;
    }

    tracing::info!("Server stopped");

    Ok(())
}
