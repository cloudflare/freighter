#!/bin/sh

# Freighter Server
export SERVER_ADDR="${SERVER_ADDR:-127.0.0.1:3000}"
export DOWNLOAD_ENDPOINT="${DOWNLOAD_ENDPOINT:-"$SERVER_ADDR/downloads/{crate}/{version}"}"
export API_ENDPOINT="${API_ENDPOINT:-"$SERVER_ADDR"}"
export METRICS_ADDR="${METRICS_ADDR:-127.0.0.1:3001}"

# PostgreSQL
export POSTGRES_HOST="${POSTGRES_HOST:?\$POSTGRES_HOST required}"
export POSTGRES_PORT="${POSTGRES_PORT:-5432}"
export POSTGRES_USER="${POSTGRES_USER:?\$POSTGRES_USER required}"
export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:?\$POSTGRES_PASSWORD required}"
export POSTGRES_DBNAME="${POSTGRES_DBNAME:-freighter}"

# S3 Storage
export BUCKET_NAME="${BUCKET_NAME:?\$BUCKET_NAME required}"
export BUCKET_ENDPOINT="${BUCKET_ENDPOINT:?\$BUCKET_ENDPOINT required}"
export BUCKET_ACCESS_KEY_ID="${BUCKET_ACCESS_KEY_ID:?\$BUCKET_ACCESS_KEY_ID required}"
export BUCKET_ACCESS_KEY_SECRET="${BUCKET_ACCESS_KEY_SECRET:?\$BUCKET_ACCESS_KEY_SECRET required}"

envsubst < "config.yaml.tpl" > "config.yaml"

exec freighter -c config.yaml
