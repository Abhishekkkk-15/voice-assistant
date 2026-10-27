pub struct TextChunker {
    buffer: String,
    is_first_sentence: bool,
    current_emotion: Option<String>,
}

impl TextChunker {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            is_first_sentence: true,
            current_emotion: None,
        }
    }

    /// Pushes a delta token and returns a complete chunk if a boundary is met.
    pub fn push(&mut self, token: &str) -> Option<String> {
        self.buffer.push_str(token);

        if let Some(split_idx) = self.find_boundary() {
            let (ready, rest) = self.buffer.split_at(split_idx);
            let raw_chunk = ready.trim().to_string();
            self.buffer = rest.to_string();

            if self.is_first_sentence {
                self.is_first_sentence = false;
            }

            if raw_chunk.is_empty() {
                return None;
            }

            return Some(self.format_chunk_with_emotion(raw_chunk));
        }

        None
    }

    /// Flushes any leftover text at the end of the LLM stream.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim().to_string();
        self.buffer.clear();
        if trimmed.is_empty() {
            None
        } else {
            Some(self.format_chunk_with_emotion(trimmed))
        }
    }

    fn format_chunk_with_emotion(&mut self, chunk: String) -> String {
        // Detect if this chunk contains an emotion tag like [whispering]
        if let Some(start) = chunk.find('[') {
            if let Some(end) = chunk[start..].find(']') {
                let tag = &chunk[start + 1..start + end];
                if !tag.trim().is_empty() {
                    self.current_emotion = Some(tag.trim().to_string());
                    return chunk;
                }
            }
        }

        // If chunk does not have an emotion tag, but we have an active emotion, prepend it for TTS
        if let Some(ref emotion) = self.current_emotion {
            format!("[{}] {}", emotion, chunk)
        } else {
            chunk
        }
    }

    fn find_boundary(&self) -> Option<usize> {
        let text = self.buffer.as_str();

        let is_inside_brackets = |pos: usize| -> bool {
            let prefix = &text[..pos];
            let open_count = prefix.chars().filter(|&c| c == '[').count();
            let close_count = prefix.chars().filter(|&c| c == ']').count();
            open_count > close_count
        };

        // 1. Initial burst heuristic: If this is the first chunk,
        // trigger on minor punctuation (e.g., ",", ":", ";") after at least 15 chars.
        if self.is_first_sentence {
            for (idx, ch) in text.char_indices() {
                if matches!(ch, ',' | ';' | ':' | '—') {
                    let end = idx + ch.len_utf8();
                    if idx >= 15 && !is_inside_brackets(end) {
                        return Some(end);
                    }
                }
            }
        }

        // 2. Standard sentence termination: '.', '!', '?', or newline '\n'
        for (idx, ch) in text.char_indices() {
            if matches!(ch, '.' | '!' | '?' | '\n') {
                let end = idx + ch.len_utf8();
                if is_inside_brackets(end) {
                    continue;
                }

                if let Some(next_ch) = text[end..].chars().next() {
                    if next_ch.is_whitespace() {
                        return Some(end);
                    }
                } else if ch == '\n' {
                    return Some(end);
                }
            }
        }

        None
    }
}
