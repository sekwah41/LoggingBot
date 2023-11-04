FROM rust:alpine AS chef
WORKDIR build
RUN \
  apk add --no-cache build-base openssl-dev; \
  cargo install cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN  cargo chef cook --release
COPY . .
RUN cargo build --release

FROM alpine
COPY --from=builder /build/target/release/logging_bot /logging_bot
CMD /logging_bot