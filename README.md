# Freighter Crate Registry
A rust private registry implementation.

## Roadmap
### v0.1.0: MVP private registry
- [X] Index
  - [x] Sparse
- [X] Crate downloads via S3-compatible buckets
- [X] UI
  - [X] Password registration
  - [X] Password login
- [X] API
  - [X] Authenticated
    - [X] Publishing
    - [X] Ownership
      - [X] Listing
      - [X] Addition
      - [X] Removal
    - [X] Yanking & un-yanking
  - [X] Searching Crates
- [X] Operational
  - [X] Logging
    - [X] Error logging
  - [X] Basic Prometheus metrics
    - [X] Request processing time
    - [X] Status code counting
