use std::string;

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
