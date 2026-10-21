use std::{env, sync::OnceLock};


pub static CONFIG: OnceLock<AppConfig> = OnceLock::new();


#[derive(Clone)]
pub struct AppConfig{
    // base url for openai client
    pub openai_base_url:String,
     // key
    pub api_key:String,    
     // chat model
    pub completion_model:String,
    pub tts_model:String,
    pub stt_model:String,
    pub tts_voice:String,
    pub azure_audio_url:String,
    pub openai_api_key:String
}





impl AppConfig{
    pub fn new()->Self{
        let key:String;
        dotenvy::dotenv().ok(); 
        let api_key_res = env::var("API_KEY");
        match  api_key_res {
            Ok(api_key  ) =>{
                key = api_key;
            },
            Err(_err) =>{
                panic!("API_KEY is required")
            }
        }

        Self{
            api_key: key,
            openai_base_url: env::var("OPENAI_BASE_URL").unwrap_or("https://centralindia.tts.speech.microsoft.com/cognitiveservices/v1".to_string()),
            completion_model: env::var("COMPLETION_MODEL").unwrap_or("gpt-5.6-luna".to_string()),
            tts_model: env::var("TTS_MODEL").unwrap_or("MAI-Voice-2".to_string()),
            stt_model: env::var("STT_MODEL").unwrap_or("something".to_string()),
            tts_voice:env::var("DEFAULT_TTS_VOICE").unwrap_or("alloy".to_string()),
            azure_audio_url:env::var("AZURE_AUDIO_SERVICE_URL").unwrap_or("alloy".to_string()),
            openai_api_key:env::var("OPENAI_API_KEY").unwrap_or("alloy".to_string()),


        }

    }

    pub fn global() -> &'static Self{
        CONFIG.get_or_init(|| AppConfig::new())
    }

}

