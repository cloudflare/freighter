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

[tracing]: https://docs.rs/tracing/latest/tracing/
[metrics]: https://docs.rs/metrics/latest/metrics/
