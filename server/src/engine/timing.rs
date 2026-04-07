use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::state::{DecodeResult, SharedState};
use crate::dsp::ft8::DecodedMessage;

/// Rolling audio buffer shared between the audio callback and this engine.
pub type AudioBuf = Arc<Mutex<Vec<f32>>>;

/// FT8 timing engine.  Runs indefinitely, triggering a decode at 13 s into
/// every 15-second UTC period (0, 15, 30, 45 seconds past the minute).
pub async fn run(state: SharedState, audio_buf: AudioBuf, sample_rate: u32) {
    loop {
        // ---- Sleep until the next decode moment --------------------------------
        let now = Utc::now();
        let now_ts = now.timestamp();

        // Find the decode moment for the current period (13 s into the period).
        let period_start = (now_ts / 15) * 15;
        let decode_ts    = period_start + 13;

        // If we already passed it, aim for the next period.
        let target_ts = if decode_ts > now_ts { decode_ts } else { decode_ts + 15 };

        let target: DateTime<Utc> = DateTime::from_timestamp(target_ts, 0)
            .unwrap_or_else(|| now + chrono::Duration::seconds(13));

        let sleep = (target - now).to_std().unwrap_or_default();
        tracing::debug!("Next FT8 decode at {} (in {:.1}s)", target, sleep.as_secs_f32());
        tokio::time::sleep(sleep).await;

        // ---- Snapshot the last 15 s of audio -----------------------------------
        let want_samples = sample_rate as usize * 15;
        let samples: Vec<f32> = {
            let buf = audio_buf.lock().unwrap_or_else(|p| p.into_inner());
            let start = buf.len().saturating_sub(want_samples);
            buf[start..].to_vec()
        };

        let period = (target_ts - 13) as u64;

        if samples.len() < sample_rate as usize * 5 {
            tracing::warn!("FT8 decode skipped: only {}s of audio buffered",
                           samples.len() / sample_rate as usize);
            continue;
        }

        tracing::info!(
            "FT8 decode: period={} samples={} ({:.1}s)",
            period,
            samples.len(),
            samples.len() as f32 / sample_rate as f32,
        );

        // ---- Decode in a blocking thread pool ----------------------------------
        let decoded: Vec<DecodedMessage> =
            tokio::task::spawn_blocking(move || crate::dsp::ft8::decode(&samples, sample_rate))
                .await
                .unwrap_or_default();

        tracing::info!("FT8 decoded {} message(s) for period {}", decoded.len(), period);

        let _ = state.decode_tx.send(DecodeResult { period, messages: decoded });
    }
}
