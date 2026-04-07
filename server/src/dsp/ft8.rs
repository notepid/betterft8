use std::ffi::CString;
use std::sync::Mutex;

use anyhow::{anyhow, Result};

/// Maximum number of decoded messages per period.
const MAX_RESULTS: usize = 50;

/// Maximum encoded samples: 15 s at 192 kHz — sized well above any realistic output rate.
const MAX_ENCODE_SAMPLES: usize = 15 * 192_000;

/// Decoded FT8 message from the C wrapper.
#[repr(C)]
struct Ft8DecodedC {
    snr:     i32,
    dt:      f32,
    freq:    f32,
    message: [i8; 36], // FT8_WRAPPER_MAX_MESSAGE
}

impl Ft8DecodedC {
    const ZERO: Self = Self {
        snr:     0,
        dt:      0.0,
        freq:    0.0,
        message: [0i8; 36],
    };
}

extern "C" {
    fn ft8_decode_audio(
        samples:     *const f32,
        num_samples: i32,
        sample_rate: i32,
        results:     *mut Ft8DecodedC,
        max_results: i32,
    ) -> i32;

    fn ft8_encode_audio(
        message_text: *const std::ffi::c_char,
        frequency:    f32,
        sample_rate:  i32,
        output:       *mut f32,
        max_samples:  i32,
    ) -> i32;
}

/// The C functions use static globals — serialise all calls with this mutex.
static CODEC_MUTEX: Mutex<()> = Mutex::new(());

/// A single decoded FT8 message.
#[derive(Clone)]
pub struct DecodedMessage {
    pub snr:     i32,
    pub dt:      f32,
    pub freq:    f32,
    pub message: String,
}

/// Decode FT8 from a slice of mono f32 audio samples.
///
/// `samples` should cover approximately 15 seconds at `sample_rate` Hz.
/// CPU-intensive; call via `tokio::task::spawn_blocking`.
pub fn decode(samples: &[f32], sample_rate: u32) -> Vec<DecodedMessage> {
    let _guard = CODEC_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

    let mut buf = [Ft8DecodedC::ZERO; MAX_RESULTS];

    let n = unsafe {
        ft8_decode_audio(
            samples.as_ptr(),
            samples.len() as i32,
            sample_rate as i32,
            buf.as_mut_ptr(),
            MAX_RESULTS as i32,
        )
    };

    let n = n.clamp(0, MAX_RESULTS as i32) as usize;

    buf[..n]
        .iter()
        .map(|r| {
            let msg: Vec<u8> = r
                .message
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect();
            DecodedMessage {
                snr:     r.snr,
                dt:      r.dt,
                freq:    r.freq,
                message: String::from_utf8_lossy(&msg).into_owned(),
            }
        })
        .collect()
}

/// Encode an FT8 message string to mono f32 audio at `sample_rate` Hz.
///
/// Returns `FT8_NN × round(sample_rate × 0.160)` samples — exactly 12.64 s at 12 kHz.
/// CPU-intensive; call via `tokio::task::spawn_blocking` when possible.
pub fn encode(message: &str, freq_hz: f32, sample_rate: u32) -> Result<Vec<f32>> {
    let _guard = CODEC_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

    let c_msg = CString::new(message)
        .map_err(|_| anyhow!("FT8 encode: message contains interior NUL"))?;

    let mut buf = vec![0f32; MAX_ENCODE_SAMPLES];

    let n = unsafe {
        ft8_encode_audio(
            c_msg.as_ptr(),
            freq_hz,
            sample_rate as i32,
            buf.as_mut_ptr(),
            MAX_ENCODE_SAMPLES as i32,
        )
    };

    if n < 0 {
        return Err(anyhow!("FT8 encode failed for message: {:?}", message));
    }

    buf.truncate(n as usize);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_length_at_12khz() {
        // 79 tones × 0.160 s/tone × 12000 Hz = 151 680 samples
        let samples = encode("CQ N0CALL AA00", 1000.0, 12000).expect("encode ok");
        assert_eq!(samples.len(), 151_680, "expected 151680 samples at 12 kHz");
    }
}
