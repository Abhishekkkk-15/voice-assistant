// src/core/vad.rs
use silero_vad_crs::{SileroVad, SileroVadError};
use std::collections::VecDeque;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHUNK_SIZE: usize = 512; // 32ms at 16kHz
pub const SPEECH_THRESHOLD: f32 = 0.6; // Probability >= 0.5 is speech
pub const PRE_SPEECH_CHUNKS: usize = 10; // ~320ms saved before speech to avoid clipping
pub const SILENCE_LIMIT_CHUNKS: usize = 20; // ~640ms pause before sentence finishes

pub const SPEECH_CONFIRM_CHUNKS: usize = 3;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VadState {
    Silence,
    Speaking,
}

#[derive(Debug, PartialEq)]
pub enum VadEvent {
    /// Fired the very first moment speech is detected (Barge-in: stop AI audio)
    SpeechStarted,
    /// Fired when user pauses long enough (Contains full 16kHz WAV file bytes!)
    SpeechEnded(Vec<u8>),
}

pub struct VadDetector {
    vad: SileroVad,
    threshold: f32,
    state: VadState,
    consecutive_silence: usize,
    silence_limit: usize,

    // tracks how many speech chunks in a row have been detected
    consecutive_speech: usize,
    speech_confirm_limit: usize,

    // Circular pre-speech memory (~320ms)
    pre_speech_buffer: VecDeque<Vec<f32>>,
    pre_speech_limit: usize,

    // Active speech audio being recorded
    active_speech_samples: Vec<f32>,

    // Staging buffer for incoming raw WebSocket bytes
    sample_buffer: Vec<f32>,
}

// Safety: `VadDetector` owns `SileroVad` which has exclusive access to its internal C model.
// It is never accessed concurrently across threads without synchronization, so it is safe to move across thread boundaries.
unsafe impl Send for VadDetector {}

impl VadDetector {
    /// Create a detector with default settings (threshold: 0.5, pause limit: ~640ms, pre-speech: ~320ms)
    pub fn new() -> Result<Self, SileroVadError> {
        let vad = SileroVad::new()?;
        Ok(Self {
            vad,
            threshold: SPEECH_THRESHOLD,
            state: VadState::Silence,
            consecutive_silence: 0,
            silence_limit: SILENCE_LIMIT_CHUNKS,
            pre_speech_buffer: VecDeque::with_capacity(PRE_SPEECH_CHUNKS),
            pre_speech_limit: PRE_SPEECH_CHUNKS,
            active_speech_samples: Vec::new(),
            sample_buffer: Vec::new(),
            consecutive_speech: 0,
            speech_confirm_limit: SPEECH_CONFIRM_CHUNKS,
        })
    }

    /// Feed raw 16-bit PCM bytes coming from WebSocket.
    /// Returns any events that occurred (e.g. SpeechStarted or SpeechEnded).
    pub fn push_pcm16(&mut self, pcm_bytes: &[u8]) -> Result<Vec<VadEvent>, SileroVadError> {
        let mut events = Vec::new();

        // 1. Convert pairs of bytes (i16) to normalized f32
        for pair in pcm_bytes.chunks_exact(2) {
            let sample_i16 = i16::from_le_bytes([pair[0], pair[1]]);
            let sample_f32 = sample_i16 as f32 / 32768.0;
            self.sample_buffer.push(sample_f32);
        }

        // 2. Process all complete 512-sample (32ms) chunks
        while self.sample_buffer.len() >= CHUNK_SIZE {
            let chunk: Vec<f32> = self.sample_buffer.drain(0..CHUNK_SIZE).collect();
            if let Some(event) = self.process_chunk(&chunk)? {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Core 32ms state machine
    pub fn process_chunk(&mut self, chunk: &[f32]) -> Result<Option<VadEvent>, SileroVadError> {
        let prob = self.vad.forward_chunk(chunk)?;
        let is_speech = prob >= self.threshold;

        match self.state {
            VadState::Silence => {
                if self.pre_speech_buffer.len() >= self.pre_speech_limit {
                    self.pre_speech_buffer.pop_front();
                }
                self.pre_speech_buffer.push_back(chunk.to_vec());
                if is_speech {
                    self.consecutive_speech += 1;
                    if self.consecutive_speech >= self.speech_confirm_limit {
                        self.state = VadState::Speaking;
                        self.consecutive_speech = 0;
                        self.consecutive_silence = 0;

                        // Drain all saved pre-speech audio (including the initial syllables!)
                        self.active_speech_samples.clear();
                        while let Some(pre) = self.pre_speech_buffer.pop_front() {
                            self.active_speech_samples.extend(pre);
                        }

                        return Ok(Some(VadEvent::SpeechStarted));
                    }
                } else {
                    self.consecutive_speech = 0;
                }
            }

            VadState::Speaking => {
                self.active_speech_samples.extend_from_slice(chunk);

                if is_speech {
                    self.consecutive_silence = 0;
                } else {
                    self.consecutive_silence += 1;

                    // Reached 20 chunks (~640ms) of unbroken silence: user finished speaking!
                    if self.consecutive_silence >= self.silence_limit {
                        self.state = VadState::Silence;
                        self.consecutive_silence = 0;
                        self.vad.reset();

                        let raw_samples = std::mem::take(&mut self.active_speech_samples);
                        let wav_bytes = samples_to_wav(&raw_samples, SAMPLE_RATE);

                        return Ok(Some(VadEvent::SpeechEnded(wav_bytes)));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn reset(&mut self) {
        self.vad.reset();
        self.state = VadState::Silence;
        self.consecutive_silence = 0;
        self.consecutive_speech = 0;
        self.pre_speech_buffer.clear();
        self.active_speech_samples.clear();
        self.sample_buffer.clear();
    }
}

/// Converts raw f32 samples to standard 16kHz 16-bit PCM WAV bytes with a 44-byte RIFF header
pub fn samples_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut pcm_bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        pcm_bytes.extend_from_slice(&sample_i16.to_le_bytes());
    }

    let pcm_len = pcm_bytes.len() as u32;
    let mut wav = Vec::with_capacity(44 + pcm_bytes.len());

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&pcm_len.to_le_bytes());
    wav.extend_from_slice(&pcm_bytes);

    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_lifecycle() {
        let mut vad = VadDetector::new().expect("Failed to initialize VAD");

        // 1. Feed 15 chunks of pure silence (15 * 1024 bytes = 15 chunks of 512 samples)
        let silence_bytes = vec![0u8; 1024];
        for _ in 0..15 {
            let events = vad.push_pcm16(&silence_bytes).unwrap();
            assert!(events.is_empty(), "Silence should not trigger any events");
        }

        // 2. WAV header verification
        let dummy_samples = vec![0.1f32; 1600]; // 100ms
        let wav = samples_to_wav(&dummy_samples, 16000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 3200); // 44-byte header + 1600 * 2 bytes
    }
}
