# Freighter
Freighter is a Rust private registry implementation designed to be, above all else, modular, fast and operationally
boring. Freighter is intended to be something you can feel comfortable deploying and then ignoring until the end of
time.

## Features

### Configurability
Freighter is configurable via a yaml file parsed at startup.

### Scalability
Freighter stores the index in PostgreSQL databases, and crate tarballs in services implementing basic elements of the
S3 API.

### Observability
Freighter exposes metrics via prometheus, and logs to stdout using `tracing`.

### Graceful Restarts
Freighter will stop accepting new requests but continue handling existing ones when a SIGTERM is received.

## Running locally

To try out Freighter **locally**, start a `postgres:14` or `postgres:15` server:
```
docker run -it -e POSTGRES_USER=freighter -e POSTGRES_PASSWORD=crates-crates-crates -p 5432:5432 -v /data:/var/lib/postgresql/data postgres:14
```

Run the migrations, e.g. with a locally installed `psql`:
```
PGPASSWORD=crates-crates-crates psql -U freighter -h localhost -f sql/init-index-db.sql
PGPASSWORD=crates-crates-crates psql -U freighter -h localhost -f sql/init-auth-db.sql
```

Next, we need an S3-compatible server. You can use an S3 emulator for testing purposes:
```
docker run -it -p 9090:9090 -e initialBuckets=crates -e validKmsKeys="arn:aws:kms:us-east-1:1234567890:key/valid-secret" -e debug=true -t adobe/s3mock
```

Finally, a config file using the above:
```yaml
service:
  address: "127.0.0.1:3000"
  download_endpoint: "127.0.0.1:3000/downloads/{crate}/{version}"
  api_endpoint: "127.0.0.1:3000"
  metrics_address: "127.0.0.1:3001"

index_db: &db
  dbname: "freighter"
  user: "freighter"
  password: "crates-crates-crates"
  host: "localhost"
  port: 5432

auth_db: *db

store:
  name: "crates"
  endpoint_url: "http://127.0.0.1:9090"
  region: "us-east-1"
  access_key_id: "1234567890"
  access_key_secret: "valid-secret"
```

Start Freighter:
```
cargo run -p freighter -- -c config.yaml
```
