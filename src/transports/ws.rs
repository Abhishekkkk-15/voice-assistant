use actix_ws::{AggregatedMessage, AggregatedMessageStream, Session};

use crate::{
    core::{
        agent::{Agent, EXPRESSIVE_VOICE_SYSTEM_PROMPT},
        pipeline::run_conversation_session,
    },
    model::{
        events::{AssistantEvent, UserEvent},
        message::{ClientMessage, WsQuery},
    },
};

pub async fn ws_transport(
    session: Session,
    mut msg_stream: AggregatedMessageStream,
    query: WsQuery,
) {
    let (inbound_tx, inbound_rx) = tokio::sync::mpsc::channel::<UserEvent>(128);
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel::<AssistantEvent>(128);

    let agent = Agent::new(EXPRESSIVE_VOICE_SYSTEM_PROMPT);
    let current_voice = query
        .voice
        .clone()
        .unwrap_or_else(|| "en-US-JennyMultilingualNeural".to_string());
    let user_id = query
        .user_id
        .clone()
        .unwrap_or_else(|| "user_1".to_string());
    println!(
        "👤 WebSocket session initialized for user [{}] with voice [{}]",
        user_id, current_voice
    );

    tokio::spawn(async move {
        run_conversation_session(inbound_rx, outbound_tx, agent, current_voice).await;
    });

    let mut session_outgoing = session.clone();
    let outgoing_task = tokio::spawn(async move {
        while let Some(event) = outbound_rx.recv().await {
            match event {
                AssistantEvent::AiResponse { text, emotion } => {
                    let json = serde_json::json!({ "ai_response": text, "emotion": emotion });
                    if session_outgoing.text(json.to_string()).await.is_err() {
                        break;
                    }
                }
                AssistantEvent::AudioChunk(bytes) => {
                    if session_outgoing.binary(bytes).await.is_err() {
                        break;
                    }
                }
                AssistantEvent::SentenceEnd => {
                    let json = serde_json::json!({ "status": "sentence_end" });
                    if session_outgoing.text(json.to_string()).await.is_err() {
                        break;
                    }
                }
                AssistantEvent::AudioEnd { status } => {
                    let json = serde_json::json!({ "status": status });
                    if session_outgoing.text(json.to_string()).await.is_err() {
                        break;
                    }
                }
                AssistantEvent::TextTranscription(txt) => {
                    let json = serde_json::json!({ "transcription": txt });
                    if session_outgoing.text(json.to_string()).await.is_err() {
                        break;
                    }
                }
                AssistantEvent::Error(err) => {
                    let json = serde_json::json!({ "error": err });
                    if session_outgoing.text(json.to_string()).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    while let Some(Ok(msg)) = msg_stream.recv().await {
        match msg {
            AggregatedMessage::Text(text) => {
                if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                    if let Some(voice) = parsed.voice {
                        let _ = inbound_tx.send(UserEvent::ChangeVoice(voice)).await;
                    }
                    if !parsed.content.trim().is_empty() {
                        let sender = parsed.user_id.as_deref().unwrap_or(&user_id);
                        println!("💬 [{}] says: {}", sender, parsed.content);
                        let _ = inbound_tx.send(UserEvent::TextMessage(parsed.content)).await;
                    }
                }
            }
            AggregatedMessage::Binary(bin) => {
                // If the receiver is dropped or lagging, don't hang the loop
                if inbound_tx.send(UserEvent::AudioChunk(bin.to_vec())).await.is_err() {
                    break;
                }
            }
            AggregatedMessage::Close(_) => break,
            _ => {}
        }
    }

    outgoing_task.abort();
}
