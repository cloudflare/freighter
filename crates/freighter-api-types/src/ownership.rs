pub mod response {
    #[cfg_attr(feature = "client", derive(serde::Deserialize))]
    #[cfg_attr(feature = "server", derive(serde::Serialize))]
    pub struct ListedOwner {
        pub id: u32,
        pub login: String,
        #[cfg_attr(any(feature = "client", feature = "server"), serde(default))]
        pub name: Option<String>,
    }
}
