FROM lukemathwalker/cargo-chef:latest-rust-1-slim-buster AS chef
WORKDIR /app/
RUN apt update && apt install -y libssl-dev pkg-config

FROM chef as planner
COPY src ./src
COPY Cargo.lock .
COPY Cargo.toml .


RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY src ./src
COPY Cargo.lock .
COPY Cargo.toml .
RUN cargo build --release

FROM debian:buster-slim
RUN apt update \
    && apt install -y openssl ca-certificates

RUN update-ca-certificates

COPY --from=builder /app/target/release/cached-btc-rpc /app/cached-btc-rpc

EXPOSE 8124
CMD ["/app/cached-btc-rpc"]




