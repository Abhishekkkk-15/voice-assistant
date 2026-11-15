use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Default)]
pub enum STTProvider {
    // Azure,
    #[default]
    Deepgram,
}

#[derive(Debug, Clone)]
pub enum STTEvents {
    Interim(String),
    Final(String),
    Error(String),
}

#[async_trait::async_trait]
pub trait StreamingSTTProvider: Send + Sync {
    async fn create_stream(
        &self,
    ) -> Result<(Sender<Vec<u8>>, Receiver<STTEvents>), Box<dyn std::error::Error + Send + Sync>>;
}
