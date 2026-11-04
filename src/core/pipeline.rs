use std::sync::Arc;

use crate::core::agent::Agent;
use crate::core::agent_loop::agent_loop;
use crate::core::messages::MessagesRegistry;
use crate::core::tools::tavily::TavilySearchTool;
use crate::core::tools::tool_registory::ToolRegistory;
use crate::core::tools::tools::WeatherTool;
use crate::model::events::{AssistantEvent, UserEvent};
use futures_util::StreamExt;
use tokio::sync::mpsc;

async fn process_ai_turn(
    text: String,
    agent: &mut Agent,
    outbound_tx: &mpsc::Sender<AssistantEvent>,
    current_voice: &str,
    memory: &mut MessagesRegistry,
    tool_registory: Arc<ToolRegistory>
) {
    let (tts_tx, mut tts_rx) = tokio::sync::mpsc::channel::<String>(32);

    let audio_client = agent.audio_client.clone();
    let voice = current_voice.to_string();
    let outbound_tx_clone = outbound_tx.clone();

    let agent_task = async {
        let _ = agent_loop(agent, text, outbound_tx.clone(), memory, &tts_tx, tool_registory).await;
        // Closing this channel so tts_rx loop can finish after synthesizing all sentences
        drop(tts_tx);
    };

    let tts_task = async {
        while let Some(sentence) = tts_rx.recv().await {
            if sentence.trim().is_empty() {
                continue;
            }

            let mut speech_stream = audio_client.tts(&sentence, &voice).await;
            while let Some(Ok(audio_chunk)) = speech_stream.next().await {
                if outbound_tx_clone
                    .send(AssistantEvent::AudioChunk(audio_chunk))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            // Sentence audio complete: signal client to start playback of this sentence immediately
            if outbound_tx_clone
                .send(AssistantEvent::SentenceEnd)
                .await
                .is_err()
            {
                return;
            }
        }
        // All sentences completed: signal final stream end
        let _ = outbound_tx_clone
            .send(AssistantEvent::AudioEnd {
                status: "audio_end".to_string(),
            })
            .await;
    };

    tokio::join!(agent_task, tts_task);
}

pub async fn run_conversation_session(
    mut inbound_rx: mpsc::Receiver<UserEvent>,
    outbound_tx: mpsc::Sender<AssistantEvent>,
    mut agent: Agent,
    mut current_voice: String,
) {
    let mut memory = MessagesRegistry::new(crate::core::agent::EXPRESSIVE_VOICE_SYSTEM_PROMPT);
    let mut tool_reg = ToolRegistory::new();
    tool_reg.register(Arc::new(WeatherTool));
    tool_reg.register(Arc::new(TavilySearchTool));
    let tool_registory_arc = Arc::new(tool_reg);
    while let Some(event) = inbound_rx.recv().await {
        match event {
            UserEvent::ChangeVoice(v) => {
                current_voice = v;
            }
            UserEvent::TextMessage(text) => {
                let reg_clone_1 = Arc::clone(&tool_registory_arc);
                process_ai_turn(text, &mut agent, &outbound_tx, &current_voice, &mut memory, reg_clone_1).await;
            }
            UserEvent::AudioChunk(bin) => {
                if bin.starts_with(b"RIFF") {
                    let reg_clone_2 = Arc::clone(&tool_registory_arc);
                    match agent.transcribe(bin).await {
                        Ok(text) if !text.trim().is_empty() => {
                            let _ = outbound_tx
                                .send(AssistantEvent::TextTranscription(text.clone()))
                                .await;
                            process_ai_turn(
                                text,
                                &mut agent,
                                &outbound_tx,
                                &current_voice,
                                &mut memory,
                                reg_clone_2
                            )
                            .await;
                        }
                        Ok(_) => {}
                        Err(err) => {
                            let _ = outbound_tx
                                .send(AssistantEvent::Error(err.to_string()))
                                .await;
                        }
                    }
                }
            }
        }
    }
}
