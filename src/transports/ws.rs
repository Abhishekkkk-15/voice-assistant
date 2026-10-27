use actix_ws::{AggregatedMessage, AggregatedMessageStream, Session};

use crate::{core::{agent::{Agent, EXPRESSIVE_VOICE_SYSTEM_PROMPT}, pipeline::run_conversation_session}, model::{events::{AssistantEvent, UserEvent}, message::{ClientMessage, WsQuery}}};



pub async fn ws_transport(
      mut session: Session,
    mut msg_stream: AggregatedMessageStream,
    query: WsQuery, 
){
 let (inbound_tx, inbound_rx) = tokio::sync::mpsc::channel::<UserEvent>(32);
 let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel::<AssistantEvent>(32);

let agent = Agent::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);
let current_voice = query.voice.unwrap_or_else(|| "en-US-JennyMultilingualNeural".to_string());
tokio::spawn(async move{
    run_conversation_session(inbound_rx, outbound_tx, agent, current_voice).await;
});

loop {
        tokio::select! {
            // A. INCOMING from Client WebSocket
            Some(Ok(msg)) = msg_stream.recv() => {
                match msg {
                    AggregatedMessage::Text(text) => {
                        if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                            if let Some(voice) = parsed.voice {
                                let _ = inbound_tx.send(UserEvent::ChangeVoice(voice)).await;
                            }
                            if !parsed.content.trim().is_empty() {
                                let _ = inbound_tx.send(UserEvent::TextMessage(parsed.content)).await;
                            }
                        }
                    }
                    AggregatedMessage::Binary(bin) => {
                        let _ = inbound_tx.send(UserEvent::AudioChunk(bin.to_vec())).await;
                    }
                    AggregatedMessage::Close(_) => break,
                    _ => {}
                }
            }

            // B. OUTGOING to Client WebSocket
            Some(event) = outbound_rx.recv() => {
                match event {
                    AssistantEvent::AiResponse { text, emotion } => {
                        let json = serde_json::json!({ "ai_response": text, "emotion": emotion });
                        let _ = session.text(json.to_string()).await;
                    }
                    AssistantEvent::AudioChunk(bytes) => {
                        let _ = session.binary(bytes).await;
                    }
                    AssistantEvent::TextTranscription(txt) => {
                        let json = serde_json::json!({ "transcription": txt });
                        let _ = session.text(json.to_string()).await;
                    }
                    AssistantEvent::AudioEnd { status } => {
                        let json = serde_json::json!({ "status": status });
                        let _ = session.text(json.to_string()).await;
                    }
                    AssistantEvent::Error(err) => {
                        let json = serde_json::json!({ "error": err });
                        let _ = session.text(json.to_string()).await;
                    }
                  
                }
            }
        }
    }


}