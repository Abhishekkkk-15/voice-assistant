use std::sync::Arc;

use crate::core::agent::Agent;
use crate::core::agent_loop::agent_loop;
use crate::core::messages::MessagesRegistry;
use crate::core::tools::tavily::TavilySearchTool;
use crate::core::tools::tool_registory::ToolRegistory;
use crate::core::tools::tools::WeatherTool;
use crate::core::vad::{VadDetector, VadEvent};
use crate::model::events::{AssistantEvent, UserEvent};
use futures_util::StreamExt;
use tokio::sync::mpsc;

async fn process_ai_turn(
    text: String,
    agent: Arc<Agent>,
    outbound_tx: mpsc::Sender<AssistantEvent>,
    voice: String,
    memory: Arc<tokio::sync::Mutex<MessagesRegistry>>,
    tool_registory: Arc<ToolRegistory>,
) {
    let (tts_tx, mut tts_rx) = tokio::sync::mpsc::channel::<String>(32);

    let audio_client = agent.audio_client.clone();
    let outbound_tx_clone = outbound_tx.clone();

    let agent_task = async {
        let mut mem_guard = memory.lock().await;
        let _ = agent_loop(
            &agent,
            text,
            outbound_tx.clone(),
            &mut *mem_guard,
            &tts_tx,
            tool_registory,
        )
        .await;
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
    agent: Agent,
    mut current_voice: String,
) {
    let memory = Arc::new(tokio::sync::Mutex::new(MessagesRegistry::new(
        crate::core::agent::EXPRESSIVE_VOICE_SYSTEM_PROMPT,
    )));
    let mut tool_reg = ToolRegistory::new();
    tool_reg.register(Arc::new(WeatherTool));
    tool_reg.register(Arc::new(TavilySearchTool));
    let tool_registory_arc = Arc::new(tool_reg);
    let agent_arc = Arc::new(agent);
    let mut vad = VadDetector::new().expect("Failed to initialize VAD");
    let mut current_ai_turn: Option<tokio::task::JoinHandle<()>> = None;

    while let Some(event) = inbound_rx.recv().await {
        match event {
            UserEvent::ChangeVoice(v) => {
                current_voice = v;
            }
            UserEvent::TextMessage(text) => {
                if let Some(handle) = current_ai_turn.take() {
                    handle.abort();
                }

                let agent_cloned = Arc::clone(&agent_arc);
                let memory_cloned = Arc::clone(&memory);
                let tool_reg_cloned = Arc::clone(&tool_registory_arc);
                let outbound_cloned = outbound_tx.clone();
                let voice = current_voice.clone();

                current_ai_turn = Some(tokio::spawn(async move {
                    process_ai_turn(
                        text,
                        agent_cloned,
                        outbound_cloned,
                        voice,
                        memory_cloned,
                        tool_reg_cloned,
                    )
                    .await;
                }));
            }
            UserEvent::AudioChunk(bin) => {
                if bin.starts_with(b"RIFF") {
                    if let Some(handle) = current_ai_turn.take() {
                        handle.abort();
                    }

                    let agent_cloned = Arc::clone(&agent_arc);
                    let memory_cloned = Arc::clone(&memory);
                    let tool_reg_cloned = Arc::clone(&tool_registory_arc);
                    let outbound_cloned = outbound_tx.clone();
                    let voice = current_voice.clone();

                    current_ai_turn = Some(tokio::spawn(async move {
                        match agent_cloned.transcribe(bin).await {
                            Ok(text) if !text.trim().is_empty() => {
                                let _ = outbound_cloned
                                    .send(AssistantEvent::TextTranscription(text.clone()))
                                    .await;
                                process_ai_turn(
                                    text,
                                    agent_cloned,
                                    outbound_cloned,
                                    voice,
                                    memory_cloned,
                                    tool_reg_cloned,
                                )
                                .await;
                            }
                            Ok(_) => {}
                            Err(err) => {
                                let _ = outbound_cloned
                                    .send(AssistantEvent::Error(err.to_string()))
                                    .await;
                            }
                        }
                    }));
                } else {
                    let events = vad.push_pcm16(&bin).unwrap_or_default();
                    for event in events {
                        match event {
                            VadEvent::SpeechStarted => {
                                println!("🗣️ User started speaking (Barge-in)");
                                if let Some(handle) = current_ai_turn.take() {
                                    println!("🛑 Aborting current AI generation/TTS turn due to barge-in");
                                    handle.abort();
                                }
                                let _ = outbound_tx
                                    .send(AssistantEvent::AudioEnd {
                                        status: "barge_in".to_string(),
                                    })
                                    .await;
                            }
                            VadEvent::SpeechEnded(wav_bytes) => {
                                println!("🤫 User finished speaking (~640ms pause), transcribing...");
                                if let Some(handle) = current_ai_turn.take() {
                                    handle.abort();
                                }

                                let agent_cloned = Arc::clone(&agent_arc);
                                let memory_cloned = Arc::clone(&memory);
                                let tool_reg_cloned = Arc::clone(&tool_registory_arc);
                                let outbound_cloned = outbound_tx.clone();
                                let voice = current_voice.clone();

                                current_ai_turn = Some(tokio::spawn(async move {
                                    match agent_cloned.transcribe(wav_bytes).await {
                                        Ok(text) if !text.trim().is_empty() => {
                                            println!("📝 Transcription: \"{text}\"");
                                            let _ = outbound_cloned
                                                .send(AssistantEvent::TextTranscription(text.clone()))
                                                .await;
                                            process_ai_turn(
                                                text,
                                                agent_cloned,
                                                outbound_cloned,
                                                voice,
                                                memory_cloned,
                                                tool_reg_cloned,
                                            )
                                            .await;
                                        }
                                        Ok(_) => {
                                            println!("⚠️ Empty transcription from VAD chunk");
                                        }
                                        Err(err) => {
                                            eprintln!("❌ STT Error: {err}");
                                            let _ = outbound_cloned
                                                .send(AssistantEvent::Error(err.to_string()))
                                                .await;
                                        }
                                    }
                                }));
                            }
                        }
                    }
                }
            }
        }
    }
}
