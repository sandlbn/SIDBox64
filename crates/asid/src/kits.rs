//! Each kit has 7 DrumVoice entries (one per track).
//! Voice assignment:
//!   SID1 v0 = Kick    SID1 v1 = Snare   SID1 v2 = ClosedHat / OpenHat (shared)
//!   SID2 v0 = Tom     SID2 v1 = Clap    SID2 v2 = Crash

use drumbox64_core::{Kit, NUM_TRACKS};

// SID waveform control bits (same as original hardware)
pub const GATE: u8 = 0x01;
pub const TRI: u8 = 0x10;
pub const SAW: u8 = 0x20;
pub const PULSE: u8 = 0x40;
pub const NOISE: u8 = 0x80;

/// Synthesis parameters for a single drum instrument.
#[derive(Debug, Clone, Copy)]
pub struct DrumVoice {
    pub freq_start: u16,
    pub freq_end: u16,
    pub waveform: u8,
    pub attack_decay: u8,
    pub sustain_release: u8,
    pub sweep_rate: u16,
    /// Gate duration in ISR ticks (~240 Hz on the original hardware).
    pub ttl: u8,
    /// Optional second waveform (Hubbard noise-then-pitched technique).
    /// 0 = no switch.
    pub wave2: u8,
    pub wave2_tick: u8,
    pub freq2: u16,
}

impl DrumVoice {
    const fn new(
        freq_start: u16,
        freq_end: u16,
        waveform: u8,
        ad: u8,
        sr: u8,
        sweep: u16,
        ttl: u8,
        wave2: u8,
        wave2_tick: u8,
        freq2: u16,
    ) -> Self {
        Self {
            freq_start,
            freq_end,
            waveform,
            attack_decay: ad,
            sustain_release: sr,
            sweep_rate: sweep,
            ttl,
            wave2,
            wave2_tick,
            freq2,
        }
    }
}

pub type KitDef = [DrumVoice; NUM_TRACKS];

/// KIT_909 – punchy electronic TR-909 style.
pub const KIT_909: KitDef = [
    // kick:  noise click then TRI pitch sweep
    DrumVoice::new(3405, 936, NOISE, 0x03, 0x09, 133, 16, TRI | GATE, 2, 3065),
    // snare: noise crack then TRI body
    DrumVoice::new(22000, 22000, NOISE, 0x01, 0x06, 0, 9, TRI | GATE, 2, 2724),
    // chh:   very high noise, ultra-short
    DrumVoice::new(32000, 32000, NOISE, 0x01, 0x01, 0, 2, 0, 0, 0),
    // ohh:   same noise, longer decay
    DrumVoice::new(32000, 32000, NOISE, 0x01, 0x07, 0, 11, 0, 0, 0),
    // tom:   noise sweep then TRI body
    DrumVoice::new(16000, 4257, NOISE, 0x02, 0x08, 30, 13, TRI | GATE, 2, 4257),
    // clap:  high noise, short
    DrumVoice::new(24000, 24000, NOISE, 0x01, 0x04, 0, 5, 0, 0, 0),
    // crash: high noise, long decay
    DrumVoice::new(30000, 30000, NOISE, 0x01, 0x0C, 0, 24, 0, 0, 0),
];

/// KIT_808 – deep boomy TR-808 style.
pub const KIT_808: KitDef = [
    DrumVoice::new(2213, 851, SAW, 0x09, 0x00, 19, 80, TRI | GATE, 2, 0),
    DrumVoice::new(12000, 12000, NOISE, 0x03, 0x00, 0, 14, TRI | GATE, 2, 2724),
    DrumVoice::new(58000, 58000, NOISE, 0x01, 0x01, 0, 4, 0, 0, 0),
    DrumVoice::new(58000, 58000, NOISE, 0x01, 0x05, 0, 22, 0, 0, 0),
    DrumVoice::new(2213, 851, PULSE, 0x05, 0x00, 14, 20, 0, 0, 0),
    DrumVoice::new(16000, 16000, NOISE, 0x01, 0x05, 0, 8, 0, 0, 0),
    DrumVoice::new(20000, 20000, NOISE, 0x02, 0x0D, 0, 38, 0, 0, 0),
];

/// KIT_ROCK – Hubbard noise-transient + pitched body style.
pub const KIT_ROCK: KitDef = [
    DrumVoice::new(6500, 100, TRI, 0x02, 0x0A, 25, 22, 0, 0, 0),
    DrumVoice::new(
        28000,
        28000,
        NOISE,
        0x01,
        0x08,
        0,
        11,
        PULSE | GATE,
        1,
        3800,
    ),
    DrumVoice::new(20000, 20000, NOISE, 0x01, 0x02, 0, 3, 0, 0, 0),
    DrumVoice::new(20000, 20000, NOISE, 0x01, 0x09, 0, 13, 0, 0, 0),
    DrumVoice::new(16000, 1396, NOISE, 0x02, 0x09, 106, 22, SAW | GATE, 2, 3746),
    DrumVoice::new(19000, 19000, NOISE, 0x01, 0x05, 0, 6, 0, 0, 0),
    DrumVoice::new(17000, 17000, NOISE, 0x02, 0x0E, 0, 38, 0, 0, 0),
];

/// KIT_SID – pure C64 game-music style.
pub const KIT_SID: KitDef = [
    DrumVoice::new(4257, 681, NOISE, 0x02, 0x00, 255, 14, PULSE | GATE, 2, 0),
    DrumVoice::new(18000, 18000, NOISE, 0x01, 0x00, 0, 5, 0, 0, 0),
    DrumVoice::new(62000, 62000, NOISE, 0x01, 0x00, 0, 2, 0, 0, 0),
    DrumVoice::new(62000, 62000, NOISE, 0x01, 0x03, 0, 12, 0, 0, 0),
    DrumVoice::new(6811, 1362, NOISE, 0x02, 0x00, 544, 10, PULSE | GATE, 2, 0),
    DrumVoice::new(20000, 20000, NOISE, 0x02, 0x00, 0, 8, 0, 0, 0),
    DrumVoice::new(55000, 55000, NOISE, 0x01, 0x0C, 0, 30, 0, 0, 0),
];

pub fn kit_for(kit: Kit) -> &'static KitDef {
    match kit {
        Kit::Tr909 => &KIT_909,
        Kit::Tr808 => &KIT_808,
        Kit::Rock => &KIT_ROCK,
        Kit::Sid => &KIT_SID,
    }
}

/// Physical voice index for each track (mirrors T2V in original sid.c).
/// Voices 0-2 → SID1, voices 3-5 → SID2.
pub const TRACK_TO_VOICE: [usize; NUM_TRACKS] = [0, 1, 2, 2, 3, 4, 5];
pub const VOICE_SID: [u8; 6] = [0, 0, 0, 1, 1, 1];
pub const VOICE_NUM: [u8; 6] = [0, 1, 2, 0, 1, 2];
