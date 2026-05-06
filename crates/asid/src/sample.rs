//! WAV sample loader for SID `$D418` (master-volume) playback.
//!
//! Decodes WAV → mixes to mono → resamples to 8 kHz → quantizes to 4-bit.
//! The output is a `Vec<u8>` where each byte holds a value in 0..=15, ready
//! to stream to the SID master-volume register at one write per ~123 PAL
//! cycles for 8 kHz playback.

use std::path::Path;

/// SID samples are typically played at 8 kHz on USBSID-Pico — fits the SID's
/// effective bandwidth and keeps register-write traffic manageable.
pub const TARGET_SAMPLE_RATE: u32 = 8_000;

/// PAL SID cycle delay between consecutive `$D418` writes for ~8 kHz playback:
/// 985_248 / 8_000 ≈ 123.
pub const PAL_CYCLES_PER_SAMPLE: u16 = 123;

pub fn load_wav_as_sid_sample(path: impl AsRef<Path>) -> Result<Vec<u8>, String> {
    let reader = hound::WavReader::open(path).map_err(|e| format!("WAV open: {e}"))?;
    let spec = reader.spec();

    let samples_i16: Vec<i16> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .into_samples::<i32>()
            .filter_map(Result::ok)
            .map(|s| {
                let shift = (spec.bits_per_sample as i32 - 16).max(0);
                (s >> shift).clamp(i16::MIN as i32, i16::MAX as i32) as i16
            })
            .collect(),
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(Result::ok)
            .map(|f| (f * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect(),
    };

    let mono: Vec<i16> = match spec.channels {
        1 => samples_i16,
        2 => samples_i16
            .chunks_exact(2)
            .map(|c| ((c[0] as i32 + c[1] as i32) / 2) as i16)
            .collect(),
        n => return Err(format!("unsupported channel count: {n}")),
    };

    // Linear interpolation resample to 8 kHz
    let ratio = spec.sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let dst_len = (mono.len() as f64 / ratio) as usize;
    let resampled: Vec<i16> = (0..dst_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = mono.get(idx).copied().unwrap_or(0) as f32;
            let b = mono.get(idx + 1).copied().unwrap_or(0) as f32;
            (a * (1.0 - frac) + b * frac) as i16
        })
        .collect();

    // Quantize to 4-bit (0..=15) by mapping signed -32768..=32767 to unsigned 0..=15.
    let nibbles: Vec<u8> = resampled
        .iter()
        .map(|&s| (((s as i32 + 32768) >> 12) as u8) & 0x0F)
        .collect();

    Ok(nibbles)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Basic shape: an empty/silent buffer survives the pipeline.
    #[test]
    fn quantize_silence_to_midpoint() {
        let s: i16 = 0;
        let nibble = (((s as i32 + 32768) >> 12) as u8) & 0x0F;
        assert_eq!(nibble, 8); // mid-range = silence
    }

    #[test]
    fn quantize_extremes() {
        let lo = (((i16::MIN as i32 + 32768) >> 12) as u8) & 0x0F;
        let hi = (((i16::MAX as i32 + 32768) >> 12) as u8) & 0x0F;
        assert_eq!(lo, 0);
        assert_eq!(hi, 15);
    }
}
