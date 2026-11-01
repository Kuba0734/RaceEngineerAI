//! Output mixer with two layers: spotter (always immediate) and crew speech (priority queue).
//! Runs inside the cpal callback: no locks, no allocation beyond pre-reserved capacity.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};

use crate::storage::settings::Priority;

pub type Pcm = Arc<[f32]>;

pub enum MixerCmd {
    /// Plays immediately on the spotter layer, replacing a running spotter clip.
    Spotter(Pcm),
    Speech(SpeechItem),
    /// Drops queued and playing crew speech (not the spotter).
    StopSpeech,
    SetMasterVolume(f32),
    SetDucking(f32),
}

pub struct SpeechItem {
    pub id: u64,
    pub pcm: Pcm,
    pub priority: Priority,
    pub expires_at: Instant,
}

#[derive(Default)]
pub struct MixerStatus {
    pub crew_speaking: AtomicBool,
    pub spotter_speaking: AtomicBool,
    /// Output level 0..1 as f32 bits (for the "radio active" orb).
    pub level: AtomicU32,
    pub last_started_id: AtomicU64,
    pub last_finished_id: AtomicU64,
}

impl MixerStatus {
    pub fn any_speaking(&self) -> bool {
        self.crew_speaking.load(Ordering::Relaxed) || self.spotter_speaking.load(Ordering::Relaxed)
    }
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

pub struct Mixer {
    rx: Receiver<MixerCmd>,
    status: Arc<MixerStatus>,
    spotter: Option<(Pcm, usize)>,
    current: Option<(SpeechItem, usize)>,
    queue: Vec<SpeechItem>,
    master: f32,
    ducking: f32,
    duck_gain: f32,
    level: f32,
}

pub fn channel() -> (Sender<MixerCmd>, Receiver<MixerCmd>) {
    crossbeam_channel::bounded(256)
}

impl Mixer {
    pub fn new(rx: Receiver<MixerCmd>, status: Arc<MixerStatus>) -> Self {
        Self { rx, status, spotter: None, current: None, queue: Vec::with_capacity(64), master: 1.0, ducking: 0.75, duck_gain: 1.0, level: 0.0 }
    }

    fn handle(&mut self, cmd: MixerCmd) {
        match cmd {
            MixerCmd::Spotter(pcm) => self.spotter = Some((pcm, 0)),
            MixerCmd::Speech(item) => {
                // Critical messages interrupt lower-priority speech; the rest queues.
                let interrupt = item.priority >= Priority::Critical && self.current.as_ref().is_some_and(|(c, _)| c.priority < item.priority);
                if interrupt {
                    if let Some((c, _)) = self.current.take() {
                        self.status.last_finished_id.store(c.id, Ordering::Relaxed);
                    }
                    self.current = Some((item, 0));
                    self.status.last_started_id.store(self.current.as_ref().map_or(0, |c| c.0.id), Ordering::Relaxed);
                } else if self.queue.len() < self.queue.capacity() {
                    // Stable insert by priority (higher first, FIFO within a priority).
                    let at = self.queue.iter().position(|q| q.priority < item.priority).unwrap_or(self.queue.len());
                    self.queue.insert(at, item);
                }
            }
            MixerCmd::StopSpeech => {
                self.queue.clear();
                self.current = None;
            }
            MixerCmd::SetMasterVolume(v) => self.master = v.clamp(0.0, 2.0),
            MixerCmd::SetDucking(d) => self.ducking = d.clamp(0.0, 1.0),
        }
    }

    fn next_item(&mut self) {
        let now = Instant::now();
        while !self.queue.is_empty() {
            let item = self.queue.remove(0);
            // Stale messages are dropped, never said late.
            if item.expires_at > now {
                self.status.last_started_id.store(item.id, Ordering::Relaxed);
                self.current = Some((item, 0));
                return;
            }
        }
    }

    /// Fills an interleaved output buffer with `channels` channels.
    pub fn fill(&mut self, out: &mut [f32], channels: usize) {
        while let Ok(cmd) = self.rx.try_recv() {
            self.handle(cmd);
        }
        if self.current.is_none() {
            self.next_item();
        }
        let ch = channels.max(1);
        let mut peak = 0.0f32;
        for frame in out.chunks_mut(ch) {
            let mut s = 0.0;
            let spotter_on = self.spotter.is_some();
            if let Some((pcm, pos)) = self.spotter.as_mut() {
                s += pcm[*pos];
                *pos += 1;
                if *pos >= pcm.len() {
                    self.spotter = None;
                }
            }
            let target = if spotter_on { 1.0 - self.ducking } else { 1.0 };
            self.duck_gain += (target - self.duck_gain) * 0.002;
            let mut finished = false;
            if let Some((item, pos)) = self.current.as_mut() {
                s += item.pcm[*pos] * self.duck_gain;
                *pos += 1;
                finished = *pos >= item.pcm.len();
            }
            if finished {
                if let Some((item, _)) = self.current.take() {
                    self.status.last_finished_id.store(item.id, Ordering::Relaxed);
                }
                self.next_item();
            }
            let v = (s * self.master).clamp(-1.0, 1.0);
            peak = peak.max(v.abs());
            for c in frame.iter_mut() {
                *c = v;
            }
        }
        self.level = self.level * 0.8 + peak * 0.2;
        self.status.level.store(self.level.to_bits(), Ordering::Relaxed);
        self.status.crew_speaking.store(self.current.is_some(), Ordering::Relaxed);
        self.status.spotter_speaking.store(self.spotter.is_some(), Ordering::Relaxed);
    }
}

/// Owns the cpal output stream on its own thread (cpal streams are not `Send`).
pub struct AudioOut {
    pub tx: Sender<MixerCmd>,
    pub status: Arc<MixerStatus>,
    pub sample_rate: u32,
    stop: Sender<()>,
}

impl AudioOut {
    pub fn start(device_name: Option<String>) -> anyhow::Result<Self> {
        let (tx, rx) = channel();
        let status = Arc::new(MixerStatus::default());
        let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);
        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<anyhow::Result<u32>>(1);
        let st = status.clone();
        std::thread::Builder::new().name("audio-out".into()).spawn(move || {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
            let run = || -> anyhow::Result<(cpal::Stream, u32)> {
                let host = cpal::default_host();
                let device = match &device_name {
                    Some(name) => host
                        .output_devices()?
                        .find(|d| d.description().map(|x| x.name() == name).unwrap_or(false))
                        .or_else(|| host.default_output_device()),
                    None => host.default_output_device(),
                }
                .ok_or_else(|| anyhow::anyhow!("no output device"))?;
                let cfg = device.default_output_config()?;
                let sr = cfg.sample_rate();
                let channels = cfg.channels() as usize;
                let mut mixer = Mixer::new(rx, st);
                let config = cfg.config();
                let err = |e| tracing::error!("audio out: {e}");
                let stream = match cfg.sample_format() {
                    cpal::SampleFormat::F32 => device.build_output_stream::<f32, _, _>(config, move |d, _| mixer.fill(d, channels), err, None)?,
                    cpal::SampleFormat::I16 => {
                        let mut buf = vec![0.0f32; 16_384];
                        device.build_output_stream::<i16, _, _>(
                            config,
                            move |d, _| {
                                if buf.len() < d.len() {
                                    buf.resize(d.len(), 0.0);
                                }
                                let b = &mut buf[..d.len()];
                                mixer.fill(b, channels);
                                for (o, s) in d.iter_mut().zip(b.iter()) {
                                    *o = (s * 32767.0) as i16;
                                }
                            },
                            err,
                            None,
                        )?
                    }
                    other => anyhow::bail!("unsupported sample format {other:?}"),
                };
                stream.play()?;
                Ok((stream, sr))
            };
            match run() {
                Ok((stream, sr)) => {
                    let _ = ready_tx.send(Ok(sr));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })?;
        let sample_rate = ready_rx.recv()??;
        Ok(Self { tx, status, sample_rate, stop: stop_tx })
    }

    pub fn send(&self, cmd: MixerCmd) {
        if self.tx.try_send(cmd).is_err() {
            tracing::warn!("mixer queue full");
        }
    }
}

impl Drop for AudioOut {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
    }
}

pub fn output_devices() -> Vec<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .output_devices()
        .map(|it| it.filter_map(|d| d.description().ok().map(|x| x.name().to_string())).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn item(id: u64, p: Priority, len: usize, ttl_ms: u64) -> SpeechItem {
        SpeechItem { id, pcm: vec![0.5; len].into(), priority: p, expires_at: Instant::now() + Duration::from_millis(ttl_ms) }
    }

    #[test]
    fn priority_order_and_expiry() {
        let (tx, rx) = channel();
        let status = Arc::new(MixerStatus::default());
        let mut m = Mixer::new(rx, status.clone());
        tx.send(MixerCmd::Speech(item(1, Priority::Info, 10, 5000))).unwrap();
        tx.send(MixerCmd::Speech(item(2, Priority::High, 10, 5000))).unwrap();
        tx.send(MixerCmd::Speech(item(3, Priority::Normal, 10, 0))).unwrap(); // expired
        let mut buf = vec![0.0; 5];
        m.fill(&mut buf, 1);
        assert_eq!(status.last_started_id.load(Ordering::Relaxed), 2);
        m.fill(&mut buf, 1);
        assert_eq!(status.last_started_id.load(Ordering::Relaxed), 1);
        // Item 3 (Normal) expired and was dropped instead of being said late; 1 plays out.
        m.fill(&mut buf, 1);
        m.fill(&mut buf, 1);
        assert!(!status.crew_speaking.load(Ordering::Relaxed));
        assert_eq!(status.last_started_id.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn critical_interrupts() {
        let (tx, rx) = channel();
        let status = Arc::new(MixerStatus::default());
        let mut m = Mixer::new(rx, status.clone());
        tx.send(MixerCmd::Speech(item(1, Priority::Normal, 1000, 5000))).unwrap();
        let mut buf = vec![0.0; 10];
        m.fill(&mut buf, 1);
        tx.send(MixerCmd::Speech(item(2, Priority::Critical, 1000, 5000))).unwrap();
        m.fill(&mut buf, 1);
        assert_eq!(status.last_started_id.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn spotter_ducks_crew() {
        let (tx, rx) = channel();
        let status = Arc::new(MixerStatus::default());
        let mut m = Mixer::new(rx, status.clone());
        tx.send(MixerCmd::Speech(item(1, Priority::Normal, 48_000, 5000))).unwrap();
        tx.send(MixerCmd::Spotter(vec![0.0; 20_000].into())).unwrap();
        let mut buf = vec![0.0; 20_000];
        m.fill(&mut buf, 1);
        // After the ramp the crew signal is reduced to ~25 %.
        assert!(buf[19_999] < 0.2, "{}", buf[19_999]);
        assert!(status.spotter_speaking.load(Ordering::Relaxed) || buf.len() == 20_000);
    }
}
