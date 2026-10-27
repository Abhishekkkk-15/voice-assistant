use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClientMessage {
    pub content: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub voice: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct WsQuery {
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub voice: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub emotion: Option<String>,
    pub text: String,
}
