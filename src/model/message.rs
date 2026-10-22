use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClientMessage {
    pub room: String,
    pub content: String,
    #[serde(default)]
    pub voice: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct WsQuery {
    pub room: String,
    pub user_id: Option<String>,
    pub voice: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub emotion: Option<String>,
    pub text: String,
}
