//! Model catalog (`resources/models.json`) and downloads into `%LOCALAPPDATA%\RaceEngineerAI\models`.
//! Downloads are verified with SHA-256 before extraction.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::storage::paths;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VoiceDef {
    pub id: String,
    pub lang: String,
    pub name: String,
    pub gender: String,
    pub license: String,
    pub tag: String,
    pub archive: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub size: u64,
    pub speakers: i32,
    /// "vits" (Piper, default) or "kokoro".
    #[serde(default)]
    pub engine: String,
    /// Names of the speakers by ID (multi-speaker models such as Kokoro).
    #[serde(default, rename = "speakerNames")]
    pub speaker_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileDef {
    pub id: String,
    pub tag: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub archive: Option<String>,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub encoder: Option<String>,
    #[serde(default)]
    pub decoder: Option<String>,
    #[serde(default)]
    pub joiner: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub base: String,
    pub voices: Vec<VoiceDef>,
    pub vad: FileDef,
    pub kws: FileDef,
}

pub fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(include_str!("../../resources/models.json")).unwrap_or_else(|e| {
            tracing::error!("models.json invalid: {e}");
            Catalog {
                base: String::new(),
                voices: Vec::new(),
                vad: FileDef { id: String::new(), tag: String::new(), file: None, archive: None, sha256: String::new(), size: 0, encoder: None, decoder: None, joiner: None },
                kws: FileDef { id: String::new(), tag: String::new(), file: None, archive: None, sha256: String::new(), size: 0, encoder: None, decoder: None, joiner: None },
            }
        })
    })
}

fn stem(archive: &str) -> &str {
    archive.strip_suffix(".tar.bz2").unwrap_or(archive)
}

pub struct VoiceFiles {
    pub model: PathBuf,
    pub tokens: PathBuf,
    pub data_dir: PathBuf,
    /// Kokoro only: speaker embeddings and lexicons.
    pub voices: PathBuf,
    pub lexicon: String,
}

impl VoiceDef {
    pub fn is_kokoro(&self) -> bool {
        self.engine == "kokoro"
    }
}

pub fn voice_files(v: &VoiceDef) -> VoiceFiles {
    let dir = paths::models_dir().join(stem(&v.archive));
    let model = if v.is_kokoro() { dir.join("model.onnx") } else { dir.join(format!("{}.onnx", v.id)) };
    // As in the official example for English (US and GB voices).
    let lexicon = [dir.join("lexicon-us-en.txt"), dir.join("lexicon-zh.txt")].iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(",");
    VoiceFiles { model, tokens: dir.join("tokens.txt"), data_dir: dir.join("espeak-ng-data"), voices: dir.join("voices.bin"), lexicon }
}

pub fn voice(id: &str) -> Option<&'static VoiceDef> {
    catalog().voices.iter().find(|v| v.id == id)
}

/// Catalog voice or a voice the user added to the models folder.
pub fn find_voice(id: &str) -> Option<VoiceDef> {
    voice(id).cloned().or_else(|| custom_voices().into_iter().find(|v| v.id == id))
}

/// Voices in the models folder that are not in the catalog: a folder `vits-piper-<id>`
/// with `<id>.onnx`, `tokens.txt` and `espeak-ng-data`.
pub fn custom_voices() -> Vec<VoiceDef> {
    let Ok(dir) = std::fs::read_dir(paths::models_dir()) else { return Vec::new() };
    let mut out: Vec<VoiceDef> = dir
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let folder = e.file_name().to_string_lossy().into_owned();
            let id = folder.strip_prefix("vits-piper-")?.to_string();
            // fp16 models do not load with the bundled runtime (see docs/audio.md).
            if id.ends_with("-fp16") || voice(&id).is_some() {
                return None;
            }
            let v = VoiceDef {
                lang: id.chars().take(2).collect::<String>().to_lowercase(),
                name: id.clone(),
                gender: String::new(),
                license: "custom".into(),
                tag: String::new(),
                archive: format!("{folder}.tar.bz2"),
                sha256: String::new(),
                size: 0,
                speakers: speaker_count(&e.path().join(format!("{id}.onnx.json"))),
                engine: String::new(),
                speaker_names: Vec::new(),
                id,
            };
            voice_installed(&v).then_some(v)
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn speaker_count(config: &Path) -> i32 {
    std::fs::read_to_string(config)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("num_speakers")?.as_i64())
        .map_or(1, |n| n.max(1) as i32)
}

/// Unpacks `vits-piper-*.tar.bz2` archives the user put into the models folder.
pub fn import_archives() -> AppResult<Vec<String>> {
    let dir = paths::models_dir();
    let mut imported = Vec::new();
    for e in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("vits-piper-") && name.ends_with(".tar.bz2") {
            extract_tar_bz2(&e.path(), &dir)?;
            std::fs::remove_file(e.path())?;
            imported.push(name);
        }
    }
    Ok(imported)
}

pub fn voice_installed(v: &VoiceDef) -> bool {
    let f = voice_files(v);
    f.model.is_file() && f.tokens.is_file() && f.data_dir.is_dir() && (!v.is_kokoro() || f.voices.is_file())
}

pub fn vad_path() -> PathBuf {
    paths::models_dir().join(catalog().vad.file.as_deref().unwrap_or("silero_vad.onnx"))
}

pub struct KwsFiles {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
}

pub fn kws_files() -> KwsFiles {
    let k = &catalog().kws;
    let dir = paths::models_dir().join(stem(k.archive.as_deref().unwrap_or("")));
    KwsFiles {
        encoder: dir.join(k.encoder.as_deref().unwrap_or("")),
        decoder: dir.join(k.decoder.as_deref().unwrap_or("")),
        joiner: dir.join(k.joiner.as_deref().unwrap_or("")),
        tokens: dir.join("tokens.txt"),
    }
}

pub fn kws_installed() -> bool {
    let f = kws_files();
    f.encoder.is_file() && f.decoder.is_file() && f.joiner.is_file() && f.tokens.is_file()
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModelStatus {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub lang: String,
    pub license: String,
    #[ts(type = "number")]
    pub size: u64,
    pub installed: bool,
    pub speakers: i32,
    #[serde(default)]
    pub speaker_names: Vec<String>,
}

pub fn status() -> Vec<ModelStatus> {
    let c = catalog();
    let mut v: Vec<ModelStatus> = c
        .voices
        .iter()
        .map(|v| ModelStatus { id: v.id.clone(), kind: "voice".into(), name: v.name.clone(), lang: v.lang.clone(), license: v.license.clone(), size: v.size, installed: voice_installed(v), speakers: v.speakers, speaker_names: v.speaker_names.clone() })
        .collect();
    v.extend(custom_voices().into_iter().map(|x| ModelStatus { id: x.id, kind: "voice".into(), name: x.name, lang: x.lang, license: x.license, size: 0, installed: true, speakers: x.speakers, speaker_names: Vec::new() }));
    v.push(ModelStatus { id: c.vad.id.clone(), kind: "vad".into(), name: "Silero VAD".into(), lang: String::new(), license: "MIT".into(), size: c.vad.size, installed: vad_path().is_file(), speakers: 0, speaker_names: Vec::new() });
    v.push(ModelStatus { id: c.kws.id.clone(), kind: "kws".into(), name: "Keyword spotter (EN)".into(), lang: "en".into(), license: "Apache-2.0".into(), size: c.kws.size, installed: kws_installed(), speakers: 0, speaker_names: Vec::new() });
    v
}

/// Downloads to a temp file, verifies SHA-256, then moves/extracts. `progress(done, total)`.
async fn download_verified(url: &str, sha256: &str, dest: &Path, progress: &(dyn Fn(u64, u64) + Send + Sync)) -> AppResult<()> {
    let client = reqwest::Client::builder().user_agent("RaceEngineerAI").build()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(0);
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp)?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk)?;
        done += chunk.len() as u64;
        progress(done, total);
    }
    file.flush()?;
    drop(file);
    let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if !got.eq_ignore_ascii_case(sha256) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Invalid(format!("checksum mismatch for {url}")));
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

fn extract_tar_bz2(archive: &Path, into: &Path) -> AppResult<()> {
    let f = std::fs::File::open(archive)?;
    let mut ar = tar::Archive::new(bzip2::read::BzDecoder::new(f));
    // `unpack` rejects entries escaping the target directory.
    ar.unpack(into)?;
    Ok(())
}

pub async fn install(id: &str, progress: &(dyn Fn(u64, u64) + Send + Sync)) -> AppResult<()> {
    let c = catalog();
    let dir = paths::models_dir();
    std::fs::create_dir_all(&dir)?;
    let (tag, name, sha) = if let Some(v) = voice(id) {
        (v.tag.as_str(), v.archive.as_str(), v.sha256.as_str())
    } else if id == c.vad.id {
        (c.vad.tag.as_str(), c.vad.file.as_deref().unwrap_or(""), c.vad.sha256.as_str())
    } else if id == c.kws.id {
        (c.kws.tag.as_str(), c.kws.archive.as_deref().unwrap_or(""), c.kws.sha256.as_str())
    } else {
        return Err(AppError::NotFound(id.into()));
    };
    let url = format!("{}/{}/{}", c.base, tag, name);
    let dest = dir.join(name);
    download_verified(&url, sha, &dest, progress).await?;
    if name.ends_with(".tar.bz2") {
        let d = dest.clone();
        let into = dir.clone();
        tokio::task::spawn_blocking(move || extract_tar_bz2(&d, &into)).await.map_err(|e| AppError::Other(e.to_string()))??;
        let _ = std::fs::remove_file(&dest);
    }
    Ok(())
}

pub fn remove(id: &str) -> AppResult<()> {
    let v = find_voice(id).ok_or_else(|| AppError::NotFound(id.into()))?;
    let dir = paths::models_dir().join(stem(&v.archive));
    if dir.starts_with(paths::models_dir()) && dir.is_dir() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_parses() {
        let c = super::catalog();
        assert!(c.voices.len() >= 4);
        assert!(c.voices.iter().all(|v| v.sha256.len() == 64));
        assert_eq!(c.kws.sha256.len(), 64);
        assert!(super::voice("de_DE-thorsten-medium").is_some());
    }
}
