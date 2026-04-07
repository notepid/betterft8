use std::f32::consts::PI;
use num_complex::Complex;

/// Apply a Hann window to a sample buffer in place.
pub fn apply_hann_window(buf: &mut [f32]) {
    let n = buf.len();
    for (i, s) in buf.iter_mut().enumerate() {
        let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / (n - 1) as f32).cos());
        *s *= w;
    }
}

/// Convert FFT output (complex spectrum) to a normalized u8 spectral line.
///
/// - `spectrum`: realfft output, length N/2+1
/// - `num_bins`: how many bins to include (0 .. num_bins)
/// - Returns a Vec<u8> of length `num_bins`, values 0–255 (low = quiet, high = loud)
pub fn spectrum_to_u8(spectrum: &[Complex<f32>], num_bins: usize) -> Vec<u8> {
    let count = num_bins.min(spectrum.len());
    let mut out = Vec::with_capacity(count);
    for bin in &spectrum[..count] {
        let power = bin.norm_sqr();
        // Add epsilon to avoid log10(0) = -inf
        let db = 10.0 * (power + 1e-12_f32).log10();
        let db = db.clamp(-120.0, 0.0);
        let normalized = ((db + 120.0) / 120.0 * 255.0) as u8;
        out.push(normalized);
    }
    out
}
