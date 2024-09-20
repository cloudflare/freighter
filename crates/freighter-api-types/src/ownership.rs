pub mod response {
    #[cfg_attr(feature = "client", derive(serde::Deserialize))]
    #[cfg_attr(feature = "server", derive(serde::Serialize))]
    pub struct OwnerList {
        // Array of owners of the crate.
        pub users: Vec<ListedOwner>,
    }

    #[cfg_attr(feature = "client", derive(serde::Deserialize))]
    #[cfg_attr(feature = "server", derive(serde::Serialize))]
    pub struct ListedOwner {
        // Unique unsigned 32-bit integer of the owner.
        pub id: u32,
        // The unique username of the owner.
        pub login: String,
        // Name of the owner.
        #[cfg_attr(any(feature = "client", feature = "server"), serde(default))]
        pub name: Option<String>,
    }

    #[cfg_attr(feature = "client", derive(serde::Deserialize))]
    #[cfg_attr(feature = "server", derive(serde::Serialize))]
    pub struct ChangedOwnership {
        // Indicates the operation succeeded, always true.
        pub ok: bool,
        // A string to be displayed to the user.
        pub msg: String,
    }

    impl ChangedOwnership {
        #[must_use]
        pub fn with_msg(msg: String) -> Self {
            Self { ok: true, msg, }
        }
    }
}
