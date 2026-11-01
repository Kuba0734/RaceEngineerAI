//! Groq API client (OpenAI compatible): Whisper transcription, streaming chat with tool calls,
//! model list. Rate-limit headers are kept for the settings page.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

const BASE: &str = "https://api.groq.com/openai/v1";

#[derive(Debug, thiserror::Error)]
pub enum GroqError {
    #[error("no API key")]
    NoKey,
    #[error("rate limited")]
    RateLimited(Option<u64>),
    #[error("unauthorized")]
    Unauthorized,
    #[error("offline: {0}")]
    Offline(String),
    #[error("api: {0}")]
    Api(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RateLimits {
    pub limit_requests: Option<u64>,
    pub remaining_requests: Option<u64>,
    pub limit_tokens: Option<u64>,
    pub remaining_tokens: Option<u64>,
    pub reset_requests: Option<String>,
    pub reset_tokens: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModelInfo {
    pub id: String,
    pub owned_by: String,
    pub context_window: Option<u32>,
    pub kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChatTurn {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Clone)]
pub struct Groq {
    client: reqwest::Client,
    pub limits: Arc<RwLock<RateLimits>>,
}

fn header_u64(h: &reqwest::header::HeaderMap, k: &str) -> Option<u64> {
    h.get(k)?.to_str().ok()?.parse().ok()
}

fn header_str(h: &reqwest::header::HeaderMap, k: &str) -> Option<String> {
    h.get(k)?.to_str().ok().map(String::from)
}

impl Default for Groq {
    fn default() -> Self {
        Self::new()
    }
}

impl Groq {
    pub fn new() -> Self {
        let client = reqwest::Client::builder().connect_timeout(Duration::from_secs(4)).timeout(Duration::from_secs(30)).user_agent("RaceEngineerAI").build().unwrap_or_default();
        Self { client, limits: Arc::new(RwLock::new(RateLimits::default())) }
    }

    fn record_limits(&self, h: &reqwest::header::HeaderMap) {
        let mut l = self.limits.write();
        if let Some(v) = header_u64(h, "x-ratelimit-limit-requests") {
            l.limit_requests = Some(v);
        }
        if let Some(v) = header_u64(h, "x-ratelimit-remaining-requests") {
            l.remaining_requests = Some(v);
        }
        if let Some(v) = header_u64(h, "x-ratelimit-limit-tokens") {
            l.limit_tokens = Some(v);
        }
        if let Some(v) = header_u64(h, "x-ratelimit-remaining-tokens") {
            l.remaining_tokens = Some(v);
        }
        if let Some(v) = header_str(h, "x-ratelimit-reset-requests") {
            l.reset_requests = Some(v);
        }
        if let Some(v) = header_str(h, "x-ratelimit-reset-tokens") {
            l.reset_tokens = Some(v);
        }
    }

    async fn check(&self, resp: reqwest::Response) -> Result<reqwest::Response, GroqError> {
        self.record_limits(resp.headers());
        match resp.status().as_u16() {
            200..=299 => Ok(resp),
            401 | 403 => Err(GroqError::Unauthorized),
            429 => Err(GroqError::RateLimited(header_u64(resp.headers(), "retry-after"))),
            code => {
                let body = resp.text().await.unwrap_or_default();
                Err(GroqError::Api(format!("{code}: {}", body.chars().take(300).collect::<String>())))
            }
        }
    }

    fn net(e: reqwest::Error) -> GroqError {
        GroqError::Offline(e.to_string())
    }

    pub async fn transcribe(&self, key: &str, wav: Vec<u8>, model: &str, language: &str, prompt: &str) -> Result<String, GroqError> {
        let part = reqwest::multipart::Part::bytes(wav).file_name("speech.wav").mime_str("audio/wav").map_err(|e| GroqError::Api(e.to_string()))?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", model.to_string())
            .text("language", language.to_string())
            .text("prompt", prompt.to_string())
            .text("response_format", "json")
            .text("temperature", "0");
        let resp = self.client.post(format!("{BASE}/audio/transcriptions")).bearer_auth(key).multipart(form).send().await.map_err(Self::net)?;
        let v: Value = self.check(resp).await?.json().await.map_err(Self::net)?;
        Ok(v.get("text").and_then(|t| t.as_str()).unwrap_or("").trim().to_string())
    }

    pub async fn models(&self, key: &str) -> Result<Vec<ModelInfo>, GroqError> {
        let resp = self.client.get(format!("{BASE}/models")).bearer_auth(key).send().await.map_err(Self::net)?;
        let v: Value = self.check(resp).await?.json().await.map_err(Self::net)?;
        let list = v.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
        let mut out: Vec<ModelInfo> = list
            .iter()
            .filter(|m| m.get("active").and_then(|a| a.as_bool()).unwrap_or(true))
            .map(|m| {
                let id = m.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let kind = if id.contains("whisper") { "stt" } else if id.contains("tts") || id.contains("playai") || id.contains("orpheus") { "tts" } else if id.contains("guard") { "guard" } else { "llm" };
                ModelInfo {
                    owned_by: m.get("owned_by").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    context_window: m.get("context_window").and_then(|x| x.as_u64()).map(|x| x as u32),
                    kind: kind.into(),
                    id,
                }
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Streams one chat completion. `on_text` receives content deltas as they arrive.
    pub async fn chat_stream(&self, key: &str, mut body: Value, on_text: &mut (dyn FnMut(&str) + Send)) -> Result<ChatTurn, GroqError> {
        body["stream"] = json!(true);
        let resp = self.client.post(format!("{BASE}/chat/completions")).bearer_auth(key).json(&body).send().await.map_err(Self::net)?;
        let resp = self.check(resp).await?;
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut turn = ChatTurn::default();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Self::net)?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(nl) = buf.find('\n') {
                let line = buf[..nl].trim().to_string();
                buf.drain(..=nl);
                let Some(data) = line.strip_prefix("data:") else { continue };
                let data = data.trim();
                if data == "[DONE]" {
                    return Ok(turn);
                }
                let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
                if let Some(err) = v.get("error") {
                    return Err(GroqError::Api(err.to_string()));
                }
                let Some(delta) = v.pointer("/choices/0/delta") else { continue };
                if let Some(t) = delta.get("content").and_then(|c| c.as_str()) {
                    turn.content.push_str(t);
                    on_text(t);
                }
                if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                    for c in calls {
                        let idx = c.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        while turn.tool_calls.len() <= idx {
                            turn.tool_calls.push(ToolCall::default());
                        }
                        let tc = &mut turn.tool_calls[idx];
                        if let Some(id) = c.get("id").and_then(|x| x.as_str()) {
                            tc.id = id.to_string();
                        }
                        if let Some(n) = c.pointer("/function/name").and_then(|x| x.as_str()) {
                            tc.name.push_str(n);
                        }
                        if let Some(a) = c.pointer("/function/arguments").and_then(|x| x.as_str()) {
                            tc.arguments.push_str(a);
                        }
                    }
                }
            }
        }
        Ok(turn)
    }
}

/// 16 kHz mono f32 → WAV (16-bit PCM) bytes.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec { channels: 1, sample_rate: 16_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cur = std::io::Cursor::new(Vec::with_capacity(samples.len() * 2 + 44));
    if let Ok(mut w) = hound::WavWriter::new(&mut cur, spec) {
        for s in samples {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let _ = w.finalize();
    }
    cur.into_inner()
}

/// Splits streamed text into speakable sentences.
#[derive(Default)]
pub struct SentenceSplitter {
    buf: String,
}

impl SentenceSplitter {
    pub fn push(&mut self, t: &str) -> Vec<String> {
        self.buf.push_str(t);
        let mut out = Vec::new();
        loop {
            let cut = self.buf.char_indices().find(|(i, c)| {
                matches!(c, '.' | '!' | '?' | '\n') && {
                    // Not a decimal point ("1.5") and the sentence has some length.
                    let next = self.buf[i + c.len_utf8()..].chars().next();
                    next.is_none_or(|n| n.is_whitespace()) && *i > 3
                }
            });
            let Some((i, c)) = cut else { break };
            let end = i + c.len_utf8();
            let next_exists = self.buf[end..].chars().next().is_some();
            if !next_exists {
                break; // wait: maybe "1." of "1.5" is still streaming
            }
            let s = self.buf[..end].trim().to_string();
            self.buf.drain(..end);
            if !s.is_empty() {
                out.push(s);
            }
        }
        out
    }

    pub fn finish(&mut self) -> Option<String> {
        let s = std::mem::take(&mut self.buf).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_streams_sentences() {
        let mut s = SentenceSplitter::default();
        let mut out = Vec::new();
        for piece in ["Sprit reicht für 1", "2 Runden. Abstand 1", ".4 nach ", "vorne! Gut"] {
            out.extend(s.push(piece));
        }
        assert_eq!(out, vec!["Sprit reicht für 12 Runden.", "Abstand 1.4 nach vorne!"]);
        assert_eq!(s.finish().unwrap(), "Gut");
    }

    #[test]
    fn wav_header() {
        let w = wav_bytes(&[0.0; 1600]);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 3200);
    }
}
