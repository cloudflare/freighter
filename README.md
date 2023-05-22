# Freeport Crate Registry

## Roadmap
### v0.2.0: Production-usable private registry
- [ ] Cloudflare Access
    - [ ] [Registry Auth](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#registry-auth)
      - [ ] Implemented here
      - [ ] Available on stable rust
    - [ ] [Credential Process](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#credential-process)
      - [ ] Implemented here
      - [ ] Available on stable rust
    - [ ] Rotating token support
- [ ] User management 2.0
  - [ ] Groups

### v0.1.0: A basic, private registry that can run behind a firewall
- [X] Index
  - [x] Sparse
- [X] Crate downloads via S3-compatible buckets
- [ ] Cache control
- [ ] UI
  - [ ] Password registration
  - [ ] Password login
- [ ] API
  - [ ] Authentication
    - [ ] Publishing
    - [ ] Owners
      - [ ] Listing
      - [ ] Addition
      - [ ] Removal
    - [ ] Yanking & unyanking
    - [ ] Password registration
    - [ ] Password login
  - [ ] Searching Crates
- [X] Operational
  - [X] Logging
    - [X] Error logging
  - [X] Basic Prometheus metrics
    - [X] Request processing time
    - [X] Status code counting
