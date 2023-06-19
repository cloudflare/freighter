use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct ListedOwner {
    pub id: u32,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
}
