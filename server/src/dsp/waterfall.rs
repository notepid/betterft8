use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use realfft::RealFftPlanner;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ringbuf::traits::{Consumer, Observer};

use crate::audio::capture::RingConsumer;
use crate::state::WaterfallLine;
use super::fft::{apply_hann_window, spectrum_to_u8};

const FFT_SIZE: usize = 4096;
const FREQ_MAX: u32 = 5000;

pub async fn run(mut cons: RingConsumer, sample_rate: u32, tx: broadcast::Sender<WaterfallLine>) {
    let mut tick = interval(Duration::from_millis(100));
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut scratch = fft.make_scratch_vec();
    let mut spectrum = fft.make_output_vec();

    // Rolling sample window: keep the last FFT_SIZE * 2 samples
    let mut window: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);

    // Bins for 0..FREQ_MAX Hz
    let num_bins = (FREQ_MAX as usize * FFT_SIZE / sample_rate as usize).min(spectrum.len());

    tracing::info!(
        "Waterfall DSP: {}Hz sample rate, {} FFT bins for 0–{}Hz",
        sample_rate, num_bins, FREQ_MAX
    );

    loop {
        tick.tick().await;

        // Drain available samples from ring buffer into the window
        let available = cons.occupied_len();
        if available > 0 {
            let before = window.len();
            window.resize(before + available, 0.0);
            cons.pop_slice(&mut window[before..]);
        }

        // Trim window to avoid unbounded growth
        if window.len() > FFT_SIZE * 2 {
            let excess = window.len() - FFT_SIZE * 2;
            window.drain(..excess);
        }

        if window.len() < FFT_SIZE {
            continue; // Not enough data yet
        }

        // Copy the newest FFT_SIZE samples and apply Hann window
        let start = window.len() - FFT_SIZE;
        let mut input: Vec<f32> = window[start..].to_vec();
        apply_hann_window(&mut input);

        // Run FFT
        if let Err(e) = fft.process_with_scratch(&mut input, &mut spectrum, &mut scratch) {
            tracing::warn!("FFT error: {e}");
            continue;
        }

        // Convert spectrum to u8 and base64-encode
        let magnitudes = spectrum_to_u8(&spectrum, num_bins);
        let data_b64 = BASE64.encode(&magnitudes);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let line = WaterfallLine {
            timestamp,
            data_b64,
            freq_min: 0,
            freq_max: FREQ_MAX,
        };

        // Ignore send errors (no subscribers yet is fine)
        let _ = tx.send(line);
    }
}
