service:
  address: "$SERVER_ADDR"
  download_endpoint: "$DOWNLOAD_ENDPOINT"
  api_endpoint: "$API_ENDPOINT"
  metrics_address: "$METRICS_ADDR"

index_db: &db
  dbname: "$POSTGRES_DBNAME"
  user: "$POSTGRES_USER"
  password: "$POSTGRES_PASSWORD"
  host: "$POSTGRES_HOST"
  port: $POSTGRES_PORT

index_path: /var/lib/freighter/index

auth_db: *db

store:
  name: "$BUCKET_NAME"
  endpoint_url: "$BUCKET_ENDPOINT"
  region: "us-east-1"
  access_key_id: "$BUCKET_ACCESS_KEY_ID"
  access_key_secret: "$BUCKET_ACCESS_KEY_ID"
