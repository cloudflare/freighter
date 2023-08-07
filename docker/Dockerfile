FROM rust:1.71-bullseye as builder
WORKDIR /usr/src/freighter

COPY Cargo.toml .
COPY Cargo.lock .
COPY .cargo/ .cargo
COPY freighter/ freighter
COPY freighter-auth/ freighter-auth
COPY freighter-index/ freighter-index
COPY freighter-server/ freighter-server
COPY freighter-storage/ freighter-storage
COPY freighter-api-types/ freighter-api-types
COPY freighter-client/ freighter-client

RUN cd freighter \
  && cargo install --path .

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/freighter /usr/local/bin/freighter

RUN apt-get update \
  && apt-get install -y gettext-base \
  && rm -rf /var/lib/apt/lists/*

COPY docker/entrypoint.sh .
COPY docker/config.yaml.tpl .

# Create file so it can be written in entrypoint
RUN touch config.yaml && chown nobody:nogroup config.yaml

USER nobody:nogroup

ENTRYPOINT ["/entrypoint.sh"]
