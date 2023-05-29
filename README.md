# Freighter Crate Registry


## Roadmap
### v0.1.0: MVP private registry
- [X] Index
  - [x] Sparse
- [X] Crate downloads via S3-compatible buckets
- [X] UI
  - [X] Password registration
  - [X] Password login
- [ ] API
  - [ ] Authenticated
    - [X] Publishing
    - [ ] Ownership
      - [ ] Listing
      - [ ] Addition
      - [ ] Removal
    - [X] Yanking & un-yanking
  - [X] Searching Crates
- [X] Operational
  - [X] Logging
    - [X] Error logging
  - [X] Basic Prometheus metrics
    - [X] Request processing time
    - [X] Status code counting
