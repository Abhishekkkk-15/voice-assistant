use tokio::sync::mpsc;
use crate::model::events::{UserEvent, AssistantEvent};
use crate::core::agent::Agent;
use crate::utlis;
use futures_util::StreamExt;


 async fn process_ai_turn(
        text: String,
        agent: &mut Agent,
        outbound_tx: &mpsc::Sender<AssistantEvent>,
        current_voice: &str,
    ) {
        match agent.generation(text).await {
            Ok(res) => {
                let Some(raw_text) = res.choices[0].message.content.clone() else {
                    let _ = outbound_tx.send(AssistantEvent::Error("AI didn't generate any text".into())).await;
                    return;
                };

                let segments = utlis::parser::parse_speech_segments(&raw_text);
                let clean_text = segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                let display_text = if clean_text.trim().is_empty() { raw_text.as_str() } else { clean_text.as_str() };
                let emotion = segments.iter().find_map(|s| s.emotion.clone()).unwrap_or_else(|| "neutral".into());

                let _ = outbound_tx.send(AssistantEvent::AiResponse {
                    text: display_text.to_string(),
                    emotion
                }).await;

                let mut speech_stream = agent.speech_generation(&raw_text, current_voice).await;
                while let Some(Ok(chunk)) = speech_stream.next().await {
                    if outbound_tx.send(AssistantEvent::AudioChunk(chunk)).await.is_err() {
                        return;
                    }
                }
            }
            Err(err) => {
                let _ = outbound_tx.send(AssistantEvent::Error(err.to_string())).await;
            }
        }
    }


pub async fn run_conversation_session(
    mut inbound_rx: mpsc::Receiver<UserEvent>,
    outbound_tx: mpsc::Sender<AssistantEvent>,
    mut agent: Agent,
    mut current_voice:String
){
 while let Some(event) = inbound_rx.recv().await{
    match event{
        UserEvent::ChangeVoice(v) => {
            current_voice = v
        },
        UserEvent::TextMessage(text) => {
         

            process_ai_turn(text, &mut agent, &outbound_tx, &current_voice).await

        },
        UserEvent::AudioChunk(bin) => {

            // Check if incoming binary data is a WAV audio file from microphone or system audio
            if bin.starts_with(b"RIFF") {
                match agent.transcribe(bin).await{
                    Ok(text) =>{
                    let _ = outbound_tx.send(AssistantEvent::TextTranscription(text.clone())).await;
                        process_ai_turn(text, &mut agent, &outbound_tx, &current_voice).await
                    },
                    Ok(_) => {},
                    Err(err) =>{
                        let _ = outbound_tx.send(AssistantEvent::Error(err.to_string())).await;
                }
                }
            }
        }
    }
 };
}