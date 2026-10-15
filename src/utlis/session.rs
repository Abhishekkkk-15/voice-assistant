use std::{collections::HashMap, string};
use tokio::sync::{broadcast, RwLock};
use std::sync::{Arc, Mutex};


// type RoomHub = Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>;
pub struct WSSession{
    pub store: Mutex<HashMap<String,broadcast::Sender<String>>>
}

// impl WSSession{



// }