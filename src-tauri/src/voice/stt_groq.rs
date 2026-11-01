//! Speech-to-text via Groq Whisper. Only segments with detected speech (0.4–15 s) are sent.

use crate::ai::groq::{wav_bytes, Groq, GroqError};
use crate::storage::settings::Settings;

pub async fn transcribe(groq: &Groq, key: &str, settings: &Settings, samples: &[f32], prompt: &str) -> Result<String, GroqError> {
    let secs = samples.len() as f32 / 16_000.0;
    let l = &settings.listening;
    if secs < l.min_speech_s || secs > l.max_speech_s.max(1.0) {
        return Ok(String::new());
    }
    let wav = wav_bytes(samples);
    let text = groq.transcribe(key, wav, &settings.groq.stt_model, settings.language.code(), prompt).await?;
    Ok(filter_hallucination(&text))
}

/// Whisper sometimes "hears" stock phrases in noise. Drop those.
pub fn filter_hallucination(t: &str) -> String {
    let n = t.trim().to_lowercase();
    const JUNK: [&str; 8] = ["untertitel", "vielen dank fürs zuschauen", "thanks for watching", "thank you for watching", "copyright", "amara.org", "subtitles by", "♪"];
    if n.chars().filter(|c| c.is_alphanumeric()).count() < 2 || JUNK.iter().any(|j| n.contains(j)) {
        String::new()
    } else {
        t.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn drops_junk() {
        assert_eq!(super::filter_hallucination("Untertitel im Auftrag des ZDF"), "");
        assert_eq!(super::filter_hallucination(" Reicht der Sprit? "), "Reicht der Sprit?");
    }
}
