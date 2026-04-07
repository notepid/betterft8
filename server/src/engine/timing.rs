use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use chrono::Utc;

use crate::engine::{logger, qso};
use crate::engine::qso::QsoState;
use crate::state::{DecodeResult, LogEntryData, QsoUpdate, SharedState, TxRequest};
use crate::dsp::ft8::DecodedMessage;

/// Rolling audio buffer shared between the audio callback and this engine.
pub type AudioBuf = Arc<std::sync::Mutex<Vec<f32>>>;

/// FT8 timing engine.  Runs indefinitely, driving both the TX and RX paths
/// of the 15-second FT8 cycle.
///
/// TX path  — fires at T-200 ms: assert PTT, queue audio, wait 12.64 s, stop.
/// RX path  — fires at T+13 s: decode audio, advance QSO state machine.
pub async fn run(state: SharedState, audio_buf: AudioBuf, sample_rate: u32) {
    loop {
        // ---- Calculate the next active period start -------------------------
        let now      = Utc::now();
        let now_secs = now.timestamp();

        let period_start   = (now_secs / 15) * 15;
        let decode_ts      = period_start + 13;
        // If decode time is already past, aim at the next period.
        let active_start   = if now_secs >= decode_ts { period_start + 15 } else { period_start };
        let active_decode_ts = active_start + 13;

        let period_parity  = ((active_start / 15) % 2) as u8;          // 0 = even, 1 = odd
        let desired_parity = state.desired_tx_parity.load(Ordering::Relaxed) as u8; // false=0, true=1

        // ---- TX path --------------------------------------------------------
        let tx_enabled = state.tx_enabled.load(Ordering::Relaxed);
        let parity_ok  = period_parity == desired_parity;
        let has_queue  = state.tx_queue.lock().await.is_some();

        if tx_enabled && parity_ok && has_queue {
            // --- Sleep until 200 ms before period start ----------------------
            let pre_ptt_ms  = active_start * 1000 - 200;
            let now_ms      = Utc::now().timestamp_millis();
            let sleep_pre   = (pre_ptt_ms - now_ms).max(0) as u64;
            tokio::time::sleep(Duration::from_millis(sleep_pre)).await;

            // Safety: if we are already more than 1 s into the period, skip TX
            // and fall through to decode so the period is not lost.
            let now_ms2 = Utc::now().timestamp_millis();
            if now_ms2 > active_start * 1000 + 1000 {
                tracing::warn!(
                    "TX: missed window ({}ms into period), will retry next period",
                    now_ms2 - active_start * 1000,
                );
                // Fall through to the decode path below.
            } else {
                do_tx(&state, active_start).await;
                // TX ran — skip decode for this TX period.
                continue;
            }
        }

        // ---- Sleep until decode time ----------------------------------------
        let now_ms     = Utc::now().timestamp_millis();
        let sleep_dec  = (active_decode_ts * 1000 - now_ms).max(0) as u64;
        tracing::debug!(
            "Next decode at T+13 of period {} (in {:.1}s)",
            active_start,
            sleep_dec as f32 / 1000.0
        );
        tokio::time::sleep(Duration::from_millis(sleep_dec)).await;

        // ---- Snapshot 15 s of audio -----------------------------------------
        let want_samples = sample_rate as usize * 15;
        let samples: Vec<f32> = {
            let buf = audio_buf.lock().unwrap_or_else(|p| p.into_inner());
            let start = buf.len().saturating_sub(want_samples);
            buf[start..].to_vec()
        };

        let period = active_start as u64;

        if samples.len() < sample_rate as usize * 5 {
            tracing::warn!(
                "FT8 decode skipped: only {}s of audio",
                samples.len() / sample_rate as usize
            );
            continue;
        }

        tracing::info!(
            "FT8 decode: period={} samples={} ({:.1}s)",
            period, samples.len(),
            samples.len() as f32 / sample_rate as f32
        );

        // ---- Decode in thread pool ------------------------------------------
        let decoded: Vec<DecodedMessage> =
            tokio::task::spawn_blocking(move || crate::dsp::ft8::decode(&samples, sample_rate))
                .await
                .unwrap_or_default();

        tracing::info!("FT8 decoded {} message(s) for period {}", decoded.len(), period);
        let result = DecodeResult { period, messages: decoded.clone() };
        let _ = state.decode_tx.send(result.clone());
        // Cache for initial state sync to newly connected clients
        {
            let mut cache = state.recent_decodes.lock().await;
            cache.push_front(result);
            if cache.len() > 5 {
                cache.pop_back();
            }
        }

        // ---- Advance QSO state machine --------------------------------------
        if state.tx_enabled.load(Ordering::Relaxed) {
            let my_call = state.config.read().unwrap().station.callsign.clone();

            // Capture InQso details before advance (needed for ADIF logging).
            let pre_qso_info = {
                let qso_guard = state.qso.lock().await;
                match &*qso_guard {
                    QsoState::InQso { their_call, their_grid, their_report, my_report, .. } => {
                        Some((their_call.clone(), their_grid.clone(), *their_report, *my_report))
                    }
                    _ => None,
                }
            };

            let next_msg = {
                let mut qso_state = state.qso.lock().await;
                qso::advance(&mut qso_state, &my_call, &decoded)
            };

            // Log QSO if it just completed.
            if pre_qso_info.is_some() && next_msg.is_none() {
                let new_state = state.qso.lock().await;
                if matches!(&*new_state, QsoState::Complete { .. }) {
                    let (their_call, their_grid, their_report, my_report) = pre_qso_info.unwrap();
                    drop(new_state); // release lock before I/O
                    maybe_log_qso(&state, their_call, their_grid, their_report, my_report).await;
                }
            }

            if let Some(msg) = next_msg {
                let tx_freq = state.qso.lock().await.tx_freq();
                let sr = state.tx_sample_rate;
                let msg2 = msg.clone();
                match tokio::task::spawn_blocking(move || {
                    crate::dsp::ft8::encode(&msg2, tx_freq, sr)
                }).await {
                    Ok(Ok(samples)) => {
                        // Queue for the next period (opposite parity)
                        *state.tx_queue.lock().await = Some(TxRequest {
                            samples,
                            message: msg.clone(),
                        });
                        tracing::info!("QSO: queued TX for next period: {}", msg);
                    }
                    Ok(Err(e)) => tracing::error!("QSO encode error: {e}"),
                    Err(e)    => tracing::error!("QSO encode task error: {e}"),
                }
            } else {
                // QSO returned None (idle or complete) — nothing more to send
            }
        }

        // ---- Broadcast QSO update -------------------------------------------
        {
            let qso   = state.qso.lock().await;
            let guard = state.tx_queue.lock().await;
            let update = QsoUpdate {
                state:      qso.clone(),
                next_tx:    guard.as_ref().map(|r| r.message.clone()),
                tx_enabled: state.tx_enabled.load(Ordering::Relaxed),
                tx_queued:  guard.is_some(),
            };
            let _ = state.qso_tx.send(update);
        }
    }
}

/// Write an ADIF log entry and broadcast a LogEntry message to all clients.
async fn maybe_log_qso(
    state:        &SharedState,
    their_call:   String,
    their_grid:   Option<String>,
    their_report: Option<i32>,
    my_report:    Option<i32>,
) {
    let freq_hz = state.last_radio_status.lock().await.freq;
    let (my_call, my_grid, log_file) = {
        let cfg = state.config.read().unwrap();
        (cfg.station.callsign.clone(), cfg.station.grid.clone(), cfg.station.log_file.clone())
    };

    let now = Utc::now();
    let qso_start = state.qso_start.lock().unwrap().unwrap_or(now);

    let fmt_snr = |v: Option<i32>| v.map(|r| format!("{:+03}", r.clamp(-99, 99))).unwrap_or_else(|| "+00".to_string());
    let rst_sent = fmt_snr(my_report);
    let rst_rcvd = fmt_snr(their_report);

    let entry = logger::QsoLogEntry {
        their_call: their_call.clone(),
        their_grid: their_grid.clone(),
        rst_sent: rst_sent.clone(),
        rst_rcvd: rst_rcvd.clone(),
        qso_start,
        qso_end: now,
        freq_hz,
        my_call,
        my_grid,
    };

    let adif = logger::AdifLogger::new(std::path::Path::new(&log_file));
    if let Err(e) = adif.log_qso(&entry) {
        tracing::error!("ADIF log error: {e}");
    } else {
        tracing::info!("QSO logged: {} → {}", their_call, log_file);
    }

    let band = logger::band_from_freq(freq_hz).to_string();
    let log_data = LogEntryData {
        their_call,
        their_grid,
        rst_sent,
        rst_rcvd,
        freq_hz,
        band,
        date:     qso_start.format("%Y%m%d").to_string(),
        time_on:  qso_start.format("%H%M%S").to_string(),
    };
    let _ = state.log_tx.send(log_data);

    // Clear the QSO start time.
    *state.qso_start.lock().unwrap() = None;
}

/// Execute a full TX cycle: PTT on → audio → PTT off.
///
/// The caller must already have verified timing is appropriate.
async fn do_tx(state: &SharedState, period_start: i64) {
    // --- Take the queued audio -----------------------------------------------
    let request = match state.tx_queue.lock().await.take() {
        Some(r) => r,
        None    => return,
    };

    tracing::info!("TX start: \"{}\"", request.message);

    // --- Wait for exact period start -----------------------------------------
    let now_ms = Utc::now().timestamp_millis();
    let sleep_ms = (period_start * 1000 - now_ms).max(0) as u64;
    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;

    // --- Assert PTT ----------------------------------------------------------
    {
        let mut guard = state.rig.lock().await;
        if let Some(rig) = guard.as_mut() {
            if let Err(e) = rig.set_ptt(true).await {
                tracing::warn!("PTT on failed: {e}");
            } else {
                tracing::info!("PTT ON");
            }
        }
    }

    // --- Start audio ---------------------------------------------------------
    if let Some(pb) = state.playback.as_ref() {
        pb.queue(request.samples);
    } else {
        tracing::warn!("No audio playback device — TX is PTT-only");
    }

    // --- Wait for transmission to complete (12.64 s) -------------------------
    // Hard limit: 13 s regardless to prevent stuck PTT
    tokio::time::sleep(Duration::from_millis(12_640)).await;

    // --- Stop audio ----------------------------------------------------------
    if let Some(pb) = state.playback.as_ref() {
        pb.cancel();
    }

    // Short gap before PTT off
    tokio::time::sleep(Duration::from_millis(110)).await;

    // --- Deassert PTT --------------------------------------------------------
    {
        let mut guard = state.rig.lock().await;
        if let Some(rig) = guard.as_mut() {
            if let Err(e) = rig.set_ptt(false).await {
                tracing::warn!("PTT off failed: {e}");
            } else {
                tracing::info!("PTT OFF");
            }
        }
    }

    tracing::info!("TX complete");
}
