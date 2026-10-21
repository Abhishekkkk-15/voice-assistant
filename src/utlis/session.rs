use std::collections::HashMap;
use tokio::sync::broadcast;
use std::sync::Mutex;


// type RoomHub = Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>;
pub struct WSSession{
    pub store: Mutex<HashMap<String,broadcast::Sender<String>>>
}

// impl WSSession{



// }