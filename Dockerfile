# Improve build speed with cached deps
ARG RUST_VERSION=1.70.0
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release

# why we dont use alpine for base image - https://andygrove.io/2020/05/why-musl-extremely-slow/
FROM debian:bookworm as runtime

COPY --from=builder /app/target/release/rustic /usr/local/bin

# usually container will be used with --rm option
# so we ignore cache
ENV RUSTIC_NO_CACHE=true

ENTRYPOINT ["/usr/local/bin/rustic"]