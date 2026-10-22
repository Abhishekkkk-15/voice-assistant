use async_openai::types::chat::ChatCompletionRequestMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    pub room: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct WsQuery{
    pub room:String,
    pub user_id:Option<String>
}


// pub struct UserAssistentMessage{
//     user: String,
//     history: Option<Vec<ChatCompletionRequestMessage>>
// }