use crate::model::message::SpeechSegment;

pub fn parse_speech_segments(input: &str) -> Vec<SpeechSegment> {
    let mut segments = Vec::new();
    let mut current_emotion: Option<String> = None;
    let mut current_text = String::new();

    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            // Read until the closing tag ']'
            let mut tag = String::new();
            let mut found_close = false;

            while let Some(&next_ch) = chars.peek() {
                chars.next();
                if next_ch == ']' {
                    found_close = true;
                    break;
                }
                tag.push(next_ch);
            }

            if found_close {
                // If there was preceding text collected for the previous emotion, flush it
                let trimmed = current_text.trim();
                if !trimmed.is_empty() {
                    segments.push(SpeechSegment {
                        emotion: current_emotion.take(),
                        text: trimmed.to_string(),
                    });
                }
                current_text.clear();
                current_emotion = Some(tag.trim().to_string());
            } else {
                // Malformed tag (e.g., unmatched '['), treat it as plain text
                current_text.push('[');
                current_text.push_str(&tag);
            }
        } else {
            current_text.push(ch);
        }
    }

    // Flush any remaining text at the end
    let trimmed = current_text.trim();
    if !trimmed.is_empty() {
        segments.push(SpeechSegment {
            emotion: current_emotion,
            text: trimmed.to_string(),
        });
    }

    segments
}

/// Helper to escape characters that break XML in SSML
fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Builds a multi-emotion SSML string for Azure TTS
pub fn build_ssml(segments: &[SpeechSegment], voice: &str) -> String {
    let mut inner_content = String::new();

    for segment in segments {
        let safe_text = escape_xml(&segment.text);

        match &segment.emotion {
            Some(style) if !style.eq_ignore_ascii_case("neutral") && !style.is_empty() => {
                inner_content.push_str(&format!(
                    r#"<mstts:express-as style="{style}" styledegree="1.8">{safe_text}</mstts:express-as>"#
                ));
            }
            _ => {
                inner_content.push_str(&safe_text);
            }
        }
        inner_content.push(' ');
    }

    format!(
        r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xmlns:mstts="https://www.w3.org/2001/mstts" xml:lang="en-US"><voice xml:lang="en-US" name="{voice}">{content}</voice></speak>"#,
        voice = voice,
        content = inner_content.trim()
    )
}
