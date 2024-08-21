# Freighter
Freighter is a Rust private registry implementation designed to be, above all else, modular, fast and operationally
boring. Freighter is intended to be something you can feel comfortable deploying and then ignoring until the end of
time.

## Design
Freighter is a modular registry.

The `freighter-server` crate provides a hyper server implementation and accepts dependency-injected authentication,
index, and storage providers, allowing users to construct customized and tailored binaries for their needs.
Authentication, index, and storage traits are found in the `freighter-auth`, `freighter-index`, and `freighter-storage`
crates. Sensible implementations (or in the case of the `yes` auth implementation, not-so-sensible implementations) can
be found in those crates, although users should feel free to provide their own implementations to suit their needs.

The Freighter network of crates produce metrics via the [metrics] crate and structured logs via the [tracing] crate.
Users rolling their own binaries can determine how and if those should be exported.

The `freighter` crate is a binary that exposes metrics and logs and hooks into postgres database(s) for authentication
and index storage, and S3-compatible services for crate storage. It is, as was previously stated, not the be-all or
end-all way to use freighter.

## Non-Goals
The desire to be operationally boring means that support for some optional things are explicit non-goals. For example,
Freighter will likely never support git indexes, as those impose significant operational concerns for users and are
difficult to design around.

Because of Freighter's modularity, it does not need to support everything out-of-the-box, so features that can be
provided via trait implementations and which I deem to be "too niche" may not be accepted. That being said, I have a
pretty limited definition of what is "too niche".

## UI
Regarding the user interface, there is none. This is largely because I am legally barred from user interface design by
international human rights law, but also because no one has gotten around to this.

If you want to contribute a user interface for Freighter, PRs are welcome!

## Running locally

To try out Freighter **locally**, start a `postgres:14` server:
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
  metrics_address: "127.0.0.1:3001"
  download_endpoint: "http://127.0.0.1:3000/downloads/{crate}/{version}"
  api_endpoint: "http://127.0.0.1:3000"

# for postgres backend
index_db: &db
  dbname: "freighter"
  user: "freighter"
  password: "crates-crates-crates"
  host: "localhost"
  port: 5432

# for filesystem backend
index_path: "/var/lib/freighter/index"

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


[tracing]: https://docs.rs/tracing/latest/tracing/
[metrics]: https://docs.rs/metrics/latest/metrics/

## Login with Cloudflare access

Edit `~/.cargo/credentials.toml`, and add at the top, if `"cargo:token"` isn't there already:

```toml
[registry]
global-credential-providers = ["cargo:token"]
```

Edit `~/.cargo/config.toml` or `.cargo/config.toml` in your project, and add:

```toml
[registries.$nickname_for_your_registry]
index = "sparse+https://$hostname_of_your_instance/index/"
```

replacing `$nickname_for_your_registry` with any name you like,
and `$hostname_of_your_instance` with the actual hostname like `registry.example.net`. It has to support HTTPS.

Then run:

```bash
cloudflared access login $hostname_of_your_instance | fgrep . | cargo login --registry=$nickname_for_your_registry
```

If you're using service auth tokens, you need to obtain a JWT from `CF_Authorization` cookie instead,
by making an HTTP request to an endpoint protected by Cloudflare Accesss. Then pipe it to the `cargo login` command above.
