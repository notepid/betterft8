use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

struct PlaybackBuf {
    samples: Vec<f32>,
    pos:     usize,
}

impl Default for PlaybackBuf {
    fn default() -> Self {
        PlaybackBuf { samples: Vec::new(), pos: 0 }
    }
}

/// Send+Sync handle to the playback buffer.  Store this in `AppState`.
///
/// The cpal `Stream` is **not** stored here because cpal marks it
/// `!Send + !Sync` on Windows (WASAPI).  Keep the `Stream` alive
/// in `main()` via the `_stream` return value of `start_playback()`.
#[derive(Clone)]
pub struct PlaybackHandle {
    /// Native output sample rate — pass this to `dsp::ft8::encode`.
    pub sample_rate: u32,
    buf: Arc<Mutex<PlaybackBuf>>,
}

impl PlaybackHandle {
    /// Queue audio samples for playback (replaces any currently queued audio).
    pub fn queue(&self, samples: Vec<f32>) {
        let mut g = self.buf.lock().unwrap();
        g.samples = samples;
        g.pos     = 0;
    }

    /// Stop playback immediately (revert to silence).
    pub fn cancel(&self) {
        let mut g = self.buf.lock().unwrap();
        g.samples.clear();
        g.pos = 0;
    }

    /// Returns true if there are still samples remaining in the buffer.
    pub fn is_playing(&self) -> bool {
        let g = self.buf.lock().unwrap();
        g.pos < g.samples.len()
    }
}

// Safety: PlaybackHandle only contains Arc<Mutex<...>> + u32, both Send+Sync.
// The cpal Stream is NOT inside this struct, so no raw-pointer concerns here.

/// Open an audio output device and return a (handle, stream) pair.
///
/// If `device_name` is `Some`, the named device is used; otherwise the system
/// default output device is selected.
///
/// The returned `cpal::Stream` must be kept alive (e.g. stored in `main`).
/// The `PlaybackHandle` is `Send + Sync` and can be placed in `AppState`.
pub fn start_playback(device_name: Option<&str>) -> Result<(PlaybackHandle, cpal::Stream)> {
    let host   = cpal::default_host();
    let device = if let Some(name) = device_name {
        host.output_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| anyhow!("audio output device '{}' not found", name))?
    } else {
        host.default_output_device()
            .ok_or_else(|| anyhow!("no default audio output device found"))?
    };

    tracing::info!(
        "Audio output device: {}",
        device.name().unwrap_or_else(|_| "unknown".into())
    );

    let default_cfg = device.default_output_config()?;
    let sample_rate = default_cfg.sample_rate().0;
    let channels    = default_cfg.channels() as usize;
    let fmt         = default_cfg.sample_format();

    tracing::info!(
        "Output audio config: {}Hz {} ch {:?}",
        sample_rate, channels, fmt
    );

    let stream_cfg = cpal::StreamConfig {
        channels:    channels as u16,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let buf: Arc<Mutex<PlaybackBuf>> = Arc::new(Mutex::new(PlaybackBuf::default()));

    fn err_fn(e: cpal::StreamError) {
        tracing::error!("Audio output error: {e}");
    }

    let stream = match fmt {
        SampleFormat::F32 => {
            let b = buf.clone();
            device.build_output_stream(
                &stream_cfg,
                move |data: &mut [f32], _| fill_output(data, channels, &b),
                err_fn, None,
            )?
        }
        SampleFormat::I16 => {
            let b = buf.clone();
            device.build_output_stream(
                &stream_cfg,
                move |data: &mut [i16], _| {
                    let mut tmp = vec![0f32; data.len()];
                    fill_output(&mut tmp, channels, &b);
                    for (d, s) in data.iter_mut().zip(tmp.iter()) {
                        *d = (*s * 32_767.0).clamp(-32_768.0, 32_767.0) as i16;
                    }
                },
                err_fn, None,
            )?
        }
        SampleFormat::I32 => {
            let b = buf.clone();
            device.build_output_stream(
                &stream_cfg,
                move |data: &mut [i32], _| {
                    let mut tmp = vec![0f32; data.len()];
                    fill_output(&mut tmp, channels, &b);
                    for (d, s) in data.iter_mut().zip(tmp.iter()) {
                        *d = (*s * 2_147_483_647.0)
                            .clamp(-2_147_483_648.0, 2_147_483_647.0) as i32;
                    }
                },
                err_fn, None,
            )?
        }
        other => return Err(anyhow!("unsupported output sample format: {:?}", other)),
    };

    stream.play()?;

    let handle = PlaybackHandle { sample_rate, buf };
    Ok((handle, stream))
}

/// Audio callback: drain mono samples from the buffer into the (possibly multi-channel) output.
fn fill_output(data: &mut [f32], channels: usize, buf: &Arc<Mutex<PlaybackBuf>>) {
    let mut g = buf.lock().unwrap();
    let mut i = 0usize;
    while i < data.len() {
        let sample = if g.pos < g.samples.len() {
            let s = g.samples[g.pos];
            g.pos += 1;
            s
        } else {
            0.0
        };
        for _ in 0..channels {
            if i < data.len() {
                data[i] = sample;
                i += 1;
            }
        }
    }
}
