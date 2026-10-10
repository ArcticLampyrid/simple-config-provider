FROM rust:alpine3.24 AS builder
RUN apk add --no-cache build-base cmake pkgconf git nodejs npm zstd
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY scripts ./scripts
RUN node scripts/prepare-llrt.mjs && cargo build --release --locked

FROM alpine:3.24
RUN apk add --no-cache ca-certificates libgcc \
    && addgroup -S provider \
    && adduser -S -D -H -u 10001 -G provider provider
COPY --from=builder /src/target/release/simple-config-provider /usr/local/bin/simple-config-provider
USER provider
ENV HOST=0.0.0.0
ENV PORT=3000
ENV CONFIG_DIR=/app/config
WORKDIR /app
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/simple-config-provider"]
