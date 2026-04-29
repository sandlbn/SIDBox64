/// Duration in milliseconds to wait before firing `next_step`.
///
/// Each step is a 16th note, so at 4/4 time sixteen steps = one bar.
/// Base duration = 15_000 / bpm ms (a quarter note is 60_000/bpm, and
/// a 16th is a quarter of that).  This matches the original C64 seq.c
/// where `ticks_per_step = 3600 / bpm` at a 240 Hz CIA timer tick.
///
/// Swing creates alternating short/long durations that sum to 2×base,
/// preserving overall tempo while producing the classic shuffle feel.
/// Even-numbered steps (0,2,4…) are shorter; odd steps (1,3,5…) are longer.
pub fn step_duration_ms(next_step: u8, bpm: u16, swing: u8) -> u64 {
    let bpm = (bpm as u64).clamp(40, 280);
    let base = 15_000u64 / bpm;
    if swing == 0 {
        return base;
    }
    let offset = (base * swing as u64 / 100).min(base.saturating_sub(4));
    if next_step % 2 == 0 {
        base.saturating_sub(offset)
    } else {
        base + offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_timing_120bpm() {
        // 16th note at 120 BPM = 125 ms
        assert_eq!(step_duration_ms(0, 120, 0), 125);
        assert_eq!(step_duration_ms(1, 120, 0), 125);
        assert_eq!(step_duration_ms(15, 120, 0), 125);
    }

    #[test]
    fn bar_length_120bpm() {
        // 16 steps at 120 BPM should sum to 2000 ms (one bar of 4/4)
        let total: u64 = (0..16).map(|s| step_duration_ms(s, 120, 0)).sum();
        assert_eq!(total, 2000);
    }

    #[test]
    fn swing_pair_sums_to_two_base() {
        for swing in [20u8, 50, 54, 80] {
            let even = step_duration_ms(0, 120, swing);
            let odd = step_duration_ms(1, 120, swing);
            assert_eq!(even + odd, 250, "swing={swing}: {even}+{odd}≠250");
        }
    }

    #[test]
    fn swing_54_classic_ratio() {
        let short = step_duration_ms(0, 120, 54) as f64;
        let long = step_duration_ms(1, 120, 54) as f64;
        let ratio = long / short;
        // Original C64 code gives ~3:1 for 54% swing at 120 BPM
        assert!(ratio > 2.5 && ratio < 4.5, "ratio={ratio:.2}");
    }

    #[test]
    fn tempo_extremes() {
        assert!(step_duration_ms(0, 40, 0) > 0);
        assert!(step_duration_ms(0, 280, 0) > 0);
    }
}
