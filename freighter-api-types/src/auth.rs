pub mod request {
    #[cfg_attr(feature = "client", derive(serde::Serialize))]
    #[cfg_attr(feature = "server", derive(serde::Deserialize))]
    pub struct AuthForm {
        pub username: String,
        pub password: String,
    }
}
