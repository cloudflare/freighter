use crate::config::Config;

pub struct ServiceState<I, S, A> {
    pub config: Config,
    pub index: I,
    pub storage: S,
    pub auth: A,
}

impl<I, S, A> ServiceState<I, S, A> {
    pub fn new(config: Config, index: I, storage: S, auth: A) -> Self {
        Self {
            config,
            index,
            storage,
            auth,
        }
    }
}
