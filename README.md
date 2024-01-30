# BTC RPC Cache Layer
Cache middleware of bitcoin RPC service  
A simple http server to cache specific btc rpc requests in memory. Useful for massive repeatedly requests to BTC rpc endpoints. 
Inspired by [cached-eth-rpc](https://github.com/tonyke-bot/cached-eth-rpc)

### Usage
With
```shell
cargo run --release -- \
  --port 8124 \
  --endpoint "bitcoin=https://btc.quiknode.pro/" 
```
Following redirection will be made:
* http://localhost:8124/ -> https://btc.quiknode.pro/

### Supported methods

- `getblock`
- `getblockhash`