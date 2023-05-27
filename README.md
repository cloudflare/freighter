# Freighter Crate Registry


## Roadmap
### v0.1.0: MVP private registry
- [X] Index
  - [x] Sparse
- [X] Crate downloads via S3-compatible buckets
- [ ] Cache control
- [ ] UI
  - [ ] Password registration
  - [ ] Password login
- [ ] API
  - [ ] Authentication
    - [X] Publishing
    - [ ] Owners
      - [ ] Listing
      - [ ] Addition
      - [ ] Removal
    - [ ] Yanking & unyanking
    - [ ] Password registration
    - [ ] Password login
  - [ ] Searching Crates
- [ ] Operational
  - [ ] E2E tests
  - [X] Logging
    - [X] Error logging
  - [X] Basic Prometheus metrics
    - [X] Request processing time
    - [X] Status code counting
