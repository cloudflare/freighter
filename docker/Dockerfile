FROM rust:1-bookworm as builder
WORKDIR /usr/src/freighter

COPY Cargo.toml .
COPY Cargo.lock .
COPY crates/ crates

RUN cargo install --path crates/freighter --features filesystem-index-backend --features cloudflare-auth-backend

FROM debian:bookworm-slim
COPY --from=builder /usr/local/cargo/bin/freighter /usr/local/bin/freighter

RUN apt-get update \
  && apt-get install -y gettext-base ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN update-ca-certificates

COPY docker/entrypoint.sh .
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Create file so it can be written in entrypoint
RUN touch config.yaml && chown nobody:nogroup config.yaml

USER nobody:nogroup

ENTRYPOINT ["/entrypoint.sh"]
