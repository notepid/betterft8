use std::sync::Mutex;

/// Maximum number of decoded messages per period.
const MAX_RESULTS: usize = 50;

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
}

/// The C decode function uses static globals; serialise all calls.
static DECODE_MUTEX: Mutex<()> = Mutex::new(());

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
    let _guard = DECODE_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

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
