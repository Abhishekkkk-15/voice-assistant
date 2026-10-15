use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    pub room: String,
    pub content: String,
}