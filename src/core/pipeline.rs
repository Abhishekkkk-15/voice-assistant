use tokio::sync::mpsc;
use crate::model::events::{UserEvent, AssistantEvent};
use crate::core::agent::Agent;
use crate::utlis::llm_buffer::TextChunker;
use futures_util::StreamExt;

async fn process_ai_turn(
    text: String,
    agent: &mut Agent,
    outbound_tx: &mpsc::Sender<AssistantEvent>,
    current_voice: &str,
) {
    let mut llm_stream = match agent.generation_stream(text).await {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("LLM Stream Error: {err:?}");
            let _ = outbound_tx.send(AssistantEvent::Error(err.to_string())).await;
            return;
        }
    };

    let (tts_tx, mut tts_rx) = tokio::sync::mpsc::channel::<String>(32);

    let full_text_collector = {
        let outbound_tx = outbound_tx.clone();
        async move {
            let mut chunker = TextChunker::new();
            let mut full_response = String::new();

            while let Some(chunk_res) = llm_stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        for choice in chunk.choices {
                            if let Some(content) = choice.delta.content {
                                full_response.push_str(&content);
                                if let Some(clause) = chunker.push(&content) {
                                    if tts_tx.send(clause).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("LLM chunk error: {err:?}");
                        let _ = outbound_tx.send(AssistantEvent::Error(err.to_string())).await;
                        break;
                    }
                }
            }

            if let Some(tail) = chunker.flush() {
                let _ = tts_tx.send(tail).await;
            }

            drop(tts_tx);

            // Send AiResponse text to client as soon as LLM generation completes
            let segments = crate::utlis::parser::parse_speech_segments(&full_response);
            let clean_text = segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
            let display_text = if clean_text.trim().is_empty() { full_response.as_str() } else { clean_text.as_str() };
            let emotion = segments.iter().find_map(|s| s.emotion.clone()).unwrap_or_else(|| "neutral".into());

            let _ = outbound_tx.send(AssistantEvent::AiResponse {
                text: display_text.to_string(),
                emotion,
            }).await;

            full_response
        }
    };

    let voice = current_voice.to_string();
    let outbound_tx_clone = outbound_tx.clone();

    let (full_text, _) = tokio::join!(
        full_text_collector,
        async {
            while let Some(sentence) = tts_rx.recv().await {
                if sentence.trim().is_empty() {
                    continue;
                }

                let mut speech_stream = agent.speech_generation(&sentence, &voice).await;
                while let Some(Ok(audio_chunk)) = speech_stream.next().await {
                    if outbound_tx_clone.send(AssistantEvent::AudioChunk(audio_chunk)).await.is_err() {
                        return;
                    }
                }
                // Sentence audio complete: signal client to start playback of this sentence immediately
                if outbound_tx_clone.send(AssistantEvent::SentenceEnd).await.is_err() {
                    return;
                }
            }
            // All sentences completed: signal final stream end
            let _ = outbound_tx_clone.send(AssistantEvent::AudioEnd { 
                status: "audio_end".to_string() 
            }).await;
        }
    );

    agent.record_assistant_response(full_text);
}

pub async fn run_conversation_session(
    mut inbound_rx: mpsc::Receiver<UserEvent>,
    outbound_tx: mpsc::Sender<AssistantEvent>,
    mut agent: Agent,
    mut current_voice: String,
) {
    while let Some(event) = inbound_rx.recv().await {
        match event {
            UserEvent::ChangeVoice(v) => {
                current_voice = v;
            }
            UserEvent::TextMessage(text) => {
                process_ai_turn(text, &mut agent, &outbound_tx, &current_voice).await;
            }
            UserEvent::AudioChunk(bin) => {
                if bin.starts_with(b"RIFF") {
                    match agent.transcribe(bin).await {
                        Ok(text) if !text.trim().is_empty() => {
                            let _ = outbound_tx.send(AssistantEvent::TextTranscription(text.clone())).await;
                            process_ai_turn(text, &mut agent, &outbound_tx, &current_voice).await;
                        }
                        Ok(_) => {}
                        Err(err) => {
                            let _ = outbound_tx.send(AssistantEvent::Error(err.to_string())).await;
                        }
                    }
                }
            }
        }
    }
}
