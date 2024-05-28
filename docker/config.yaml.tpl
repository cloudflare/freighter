service:
  address: "$SERVER_ADDR"
  download_endpoint: "$DOWNLOAD_ENDPOINT"
  api_endpoint: "$API_ENDPOINT"
  metrics_address: "$METRICS_ADDR"
  auth_required: false

index_db: &db
  dbname: "$POSTGRES_DBNAME"
  user: "$POSTGRES_USER"
  password: "$POSTGRES_PASSWORD"
  host: "$POSTGRES_HOST"
  port: $POSTGRES_PORT

index_path: /var/lib/freighter/index

auth_db: *db

# FS-based auth
auth_path: /var/lib/freighter/auth
auth_tokens_pepper: "AAAAAAAAAAxAAAAAAAAARJgA"

# Cloudflare Access
auth_audience: "<see application overview tab for the audience tag>"
auth_team_base_url: "https://<your team here>.cloudflareaccess.com"

store:
  name: "$BUCKET_NAME"
  endpoint_url: "$BUCKET_ENDPOINT"
  region: "us-east-1"
  access_key_id: "$BUCKET_ACCESS_KEY_ID"
  access_key_secret: "$BUCKET_ACCESS_KEY_ID"
