//! Microphone capture on its own thread; delivers mono f32 chunks at the device rate.

use crossbeam_channel::{Receiver, Sender};

pub struct Capture {
    pub rx: Receiver<Vec<f32>>,
    pub sample_rate: u32,
    stop: Sender<()>,
}

impl Capture {
    pub fn start(device_name: Option<String>) -> anyhow::Result<Self> {
        let (tx, rx) = crossbeam_channel::bounded::<Vec<f32>>(64);
        let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);
        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<anyhow::Result<u32>>(1);
        std::thread::Builder::new().name("mic".into()).spawn(move || {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
            let run = || -> anyhow::Result<(cpal::Stream, u32)> {
                let host = cpal::default_host();
                let device = match &device_name {
                    Some(n) => host.input_devices()?.find(|d| d.description().map(|x| x.name() == n).unwrap_or(false)).or_else(|| host.default_input_device()),
                    None => host.default_input_device(),
                }
                .ok_or_else(|| anyhow::anyhow!("no input device"))?;
                let cfg = device.default_input_config()?;
                let sr = cfg.sample_rate();
                let ch = cfg.channels() as usize;
                let config = cfg.config();
                let err = |e| tracing::error!("mic: {e}");
                let mono = move |d: &[f32]| -> Vec<f32> { d.chunks(ch.max(1)).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect() };
                let stream = match cfg.sample_format() {
                    cpal::SampleFormat::F32 => {
                        let tx = tx.clone();
                        device.build_input_stream::<f32, _, _>(config, move |d, _| { let _ = tx.try_send(mono(d)); }, err, None)?
                    }
                    cpal::SampleFormat::I16 => {
                        let tx = tx.clone();
                        device.build_input_stream::<i16, _, _>(
                            config,
                            move |d, _| {
                                let f: Vec<f32> = d.iter().map(|s| *s as f32 / 32768.0).collect();
                                let _ = tx.try_send(mono(&f));
                            },
                            err,
                            None,
                        )?
                    }
                    other => anyhow::bail!("unsupported mic format {other:?}"),
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
        Ok(Self { rx, sample_rate, stop: stop_tx })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
    }
}

pub fn input_devices() -> Vec<String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host().input_devices().map(|it| it.filter_map(|d| d.description().ok().map(|x| x.name().to_string())).collect()).unwrap_or_default()
}

/// Streaming linear resampler to 16 kHz (keeps fractional position across chunks).
pub struct Resampler16k {
    ratio: f64,
    pos: f64,
    last: f32,
}

impl Resampler16k {
    pub fn new(from: u32) -> Self {
        Self { ratio: f64::from(from) / 16_000.0, pos: 0.0, last: 0.0 }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if input.is_empty() {
            return;
        }
        // Linear interpolation; speech energy above 8 kHz is negligible for STT.
        while self.pos < input.len() as f64 {
            let i = self.pos.floor() as isize;
            let frac = (self.pos - i as f64) as f32;
            let a = if i - 1 < 0 { self.last } else { input[(i - 1) as usize] };
            let b = input[i.max(0) as usize];
            out.push(a + (b - a) * frac);
            self.pos += self.ratio;
        }
        self.pos -= input.len() as f64;
        self.last = input[input.len() - 1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_to_16k() {
        let mut r = Resampler16k::new(48_000);
        let mut out = Vec::new();
        for _ in 0..100 {
            r.process(&[0.5; 480], &mut out);
        }
        assert!((out.len() as i32 - 16_000).abs() <= 2, "{}", out.len());
    }
}
