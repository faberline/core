use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogEntry {
    pub key: String,
    pub value: Vec<u8>,
}
