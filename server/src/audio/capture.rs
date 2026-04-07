use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use ringbuf::HeapRb;
use ringbuf::traits::{Split, Producer};

use crate::config::AudioConfig;

pub type RingConsumer = ringbuf::HeapCons<f32>;

/// Start audio capture. Returns the ring buffer consumer, effective sample rate,
/// and the cpal Stream (must be kept alive).
pub fn start_capture(config: &AudioConfig) -> Result<(RingConsumer, u32, cpal::Stream)> {
    let host = cpal::default_host();

    let device = if let Some(name) = &config.input_device {
        host.input_devices()?
            .find(|d| d.name().map(|n| &n == name).unwrap_or(false))
            .ok_or_else(|| anyhow!("audio device '{}' not found", name))?
    } else {
        host.default_input_device()
            .ok_or_else(|| anyhow!("no default audio input device available"))?
    };

    tracing::info!("Audio input device: {}", device.name().unwrap_or_else(|_| "unknown".into()));

    let default_cfg = device.default_input_config()?;
    let native_rate = default_cfg.sample_rate().0;
    let native_channels = default_cfg.channels() as usize;
    let sample_format = default_cfg.sample_format();

    tracing::info!(
        "Native audio config: {}Hz, {} ch, {:?}",
        native_rate, native_channels, sample_format
    );

    let target_rate = config.sample_rate;

    // Compute decimation factor — only works cleanly when native_rate is a multiple.
    let (effective_rate, decimate_factor) = if native_rate % target_rate == 0 {
        (target_rate, (native_rate / target_rate) as usize)
    } else {
        tracing::warn!(
            "Cannot cleanly decimate {}Hz to {}Hz, using native rate for FFT",
            native_rate, target_rate
        );
        (native_rate, 1)
    };

    tracing::info!("Effective sample rate: {}Hz (decimation: {}x)", effective_rate, decimate_factor);

    // Ring buffer: 20 seconds at effective rate
    let rb = HeapRb::<f32>::new(effective_rate as usize * 20);
    let (prod, cons) = rb.split();

    let stream_config = cpal::StreamConfig {
        channels: native_channels as u16,
        sample_rate: cpal::SampleRate(native_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    fn err_fn(err: cpal::StreamError) {
        tracing::error!("Audio stream error: {}", err);
    }

    let stream = match sample_format {
        SampleFormat::F32 => {
            let mut prod = prod;
            let mut mono_acc = 0f32;
            let mut mono_count = 0usize;
            let mut dec_count = 0usize;
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    write_samples(data, native_channels, decimate_factor,
                        &mut prod, &mut mono_acc, &mut mono_count, &mut dec_count);
                },
                err_fn, None,
            )?
        }
        SampleFormat::I16 => {
            let mut prod = prod;
            let mut mono_acc = 0f32;
            let mut mono_count = 0usize;
            let mut dec_count = 0usize;
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    let converted: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    write_samples(&converted, native_channels, decimate_factor,
                        &mut prod, &mut mono_acc, &mut mono_count, &mut dec_count);
                },
                err_fn, None,
            )?
        }
        SampleFormat::I32 => {
            let mut prod = prod;
            let mut mono_acc = 0f32;
            let mut mono_count = 0usize;
            let mut dec_count = 0usize;
            device.build_input_stream(
                &stream_config,
                move |data: &[i32], _| {
                    let converted: Vec<f32> = data.iter().map(|&s| s as f32 / 2_147_483_648.0).collect();
                    write_samples(&converted, native_channels, decimate_factor,
                        &mut prod, &mut mono_acc, &mut mono_count, &mut dec_count);
                },
                err_fn, None,
            )?
        }
        SampleFormat::U16 => {
            let mut prod = prod;
            let mut mono_acc = 0f32;
            let mut mono_count = 0usize;
            let mut dec_count = 0usize;
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| {
                    let converted: Vec<f32> = data.iter()
                        .map(|&s| (s as f32 - 32768.0) / 32768.0)
                        .collect();
                    write_samples(&converted, native_channels, decimate_factor,
                        &mut prod, &mut mono_acc, &mut mono_count, &mut dec_count);
                },
                err_fn, None,
            )?
        }
        fmt => return Err(anyhow!("unsupported sample format: {:?}", fmt)),
    };

    stream.play()?;
    Ok((cons, effective_rate, stream))
}

fn write_samples(
    data: &[f32],
    channels: usize,
    decimate: usize,
    prod: &mut impl Producer<Item = f32>,
    mono_acc: &mut f32,
    mono_count: &mut usize,
    dec_count: &mut usize,
) {
    for &sample in data {
        *mono_acc += sample;
        *mono_count += 1;
        if *mono_count == channels {
            let mono = *mono_acc / channels as f32;
            *mono_acc = 0.0;
            *mono_count = 0;
            *dec_count += 1;
            if *dec_count >= decimate {
                *dec_count = 0;
                let _ = prod.try_push(mono);
            }
        }
    }
}
