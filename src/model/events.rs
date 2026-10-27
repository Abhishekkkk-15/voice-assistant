#[derive(Debug)]
pub enum UserEvent {
    TextMessage(String),
    AudioChunk(Vec<u8>),
    ChangeVoice(String),
}

#[derive(Debug)]
pub enum AssistantEvent {
    TextTranscription(String),
    AiResponse { text: String, emotion: String },
    AudioChunk(bytes::Bytes),
    SentenceEnd,
    AudioEnd { status: String },
    Error(String),
}
