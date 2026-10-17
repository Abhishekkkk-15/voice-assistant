use async_openai::Client;
use async_openai::config::{Config, OpenAIConfig};
use async_openai::types::completions::CreateCompletionRequest;
use crate::utlis::config::AppConfig;
pub struct Agent {
    /// The core OpenAI client used to communicate with all endpoints
    client: Client<OpenAIConfig>,
    
    // --- Model Configurations ---
    /// The STT model (e.g., "whisper-1")
    stt_model: String,
    /// The Text Generation model (e.g., "gpt-4o", "gpt-4o-mini")
    llm_model: String,
    /// The TTS model (e.g., "tts-1", "tts-1-hd")
    tts_model: String,
    /// The selected voice for TTS (e.g., "alloy", "echo", "nova")
    tts_voice: String,

    // --- State & Memory ---
    /// System prompt setting the assistant's personality
    system_prompt: String,
    /// The sliding window memory of the conversation
    history: Vec<CreateCompletionRequest>,
}


impl Agent{
    fn new(system_prompt:&str) -> Self {
        let config = &AppConfig::global();
   
        Self { client: Client::with_config(OpenAIConfig::default().with_api_base(config.openai_base_url.clone()).with_api_key(config.api_key.clone())), stt_model: config.stt_model.to_string(), llm_model: config.completion_model.to_string(), tts_model: config.tts_model.to_string(), tts_voice: config.tts_voice.to_string(), system_prompt: system_prompt.to_string(), history: Vec::new() }
    }
}
