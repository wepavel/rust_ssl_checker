# ------------------------- STAGE 1: Build -------------------------
FROM rust:latest AS builder
WORKDIR /app

RUN rustup target add x86_64-unknown-linux-musl && \
    apt update && apt install -y musl-tools

COPY Cargo.toml Cargo.lock ./
COPY servers.json servers.json ./
COPY base  base/
COPY checker checker/

RUN cargo fetch

RUN cargo build --release --target x86_64-unknown-linux-musl
RUN strip target/x86_64-unknown-linux-musl/release/checker

## ------------------------- STAGE 2: Final image -------------------------
FROM alpine:latest AS release
WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/checker .
COPY config.yml .
ENV CONFIG_PATH=./config.yml

CMD [ "./checker" ]

