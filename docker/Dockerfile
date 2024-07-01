FROM rust:1.79-bookworm as builder
WORKDIR /usr/src/freighter

COPY Cargo.toml .
COPY Cargo.lock .
COPY .cargo/ .cargo
COPY crates/ crates

RUN cargo install --path crates/freighter --features filesystem-index-backend

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/freighter /usr/local/bin/freighter

RUN apt-get update \
  && apt-get install -y gettext-base \
  && rm -rf /var/lib/apt/lists/*

COPY docker/entrypoint.sh .

# Create file so it can be written in entrypoint
RUN touch config.yaml && chown nobody:nogroup config.yaml

USER nobody:nogroup

ENTRYPOINT ["/entrypoint.sh"]
