use crate::config::ServiceConfig;

pub struct ServiceState<I, S, A> {
    pub config: ServiceConfig,
    pub index: I,
    pub storage: S,
    pub auth: A,
}

impl<I, S, A> ServiceState<I, S, A> {
    pub fn new(config: ServiceConfig, index: I, storage: S, auth: A) -> Self {
        Self {
            config,
            index,
            storage,
            auth,
        }
    }
}
