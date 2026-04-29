pub const NUM_TRACKS: usize = 7;
pub const NUM_STEPS: usize = 16;
pub const TEMPO_MIN: u16 = 40;
pub const TEMPO_MAX: u16 = 280;

// ── Velocity ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Velocity {
    #[default]
    Off = 0,
    Soft = 1,
    Medium = 2,
    Loud = 3,
}

impl Velocity {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Soft,
            2 => Self::Medium,
            3 => Self::Loud,
            _ => Self::Off,
        }
    }

    /// Cycles Off→Loud→Medium→Soft→Off on each click (matches original app feel).
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::Loud,
            Self::Loud => Self::Medium,
            Self::Medium => Self::Soft,
            Self::Soft => Self::Off,
        }
    }

    pub fn is_active(self) -> bool {
        self != Self::Off
    }

    pub fn midi_velocity(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Soft => 40,
            Self::Medium => 80,
            Self::Loud => 127,
        }
    }

    /// SID master-volume level used in the original hardware (0-15).
    pub fn sid_vol(self) -> u8 {
        match self {
            Self::Off => 0x00,
            Self::Soft => 0x06,
            Self::Medium => 0x0B,
            Self::Loud => 0x0F,
        }
    }
}

// ── Track ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Track {
    Kick = 0,
    Snare = 1,
    ClosedHat = 2,
    OpenHat = 3,
    Tom = 4,
    Clap = 5,
    Crash = 6,
}

impl Track {
    pub const ALL: [Track; NUM_TRACKS] = [
        Track::Kick,
        Track::Snare,
        Track::ClosedHat,
        Track::OpenHat,
        Track::Tom,
        Track::Clap,
        Track::Crash,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Kick => "Kick",
            Self::Snare => "Snare",
            Self::ClosedHat => "C.Hat",
            Self::OpenHat => "O.Hat",
            Self::Tom => "Tom",
            Self::Clap => "Clap",
            Self::Crash => "Crash",
        }
    }

    /// GM drum map note numbers (MIDI channel 10).
    pub fn midi_note(self) -> u8 {
        match self {
            Self::Kick => 36,      // Bass Drum 1
            Self::Snare => 38,     // Acoustic Snare
            Self::ClosedHat => 42, // Closed Hi-Hat
            Self::OpenHat => 46,   // Open Hi-Hat
            Self::Tom => 45,       // Low Floor Tom
            Self::Clap => 39,      // Hand Clap
            Self::Crash => 49,     // Crash Cymbal 1
        }
    }

    pub fn from_index(i: usize) -> Option<Self> {
        Self::ALL.get(i).copied()
    }
}

// ── Kit ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Kit {
    #[default]
    Tr909 = 0,
    Tr808 = 1,
    Rock = 2,
    Sid = 3,
}

impl Kit {
    pub const ALL: [Kit; 4] = [Kit::Tr909, Kit::Tr808, Kit::Rock, Kit::Sid];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tr909 => "909",
            Self::Tr808 => "808",
            Self::Rock => "Rock",
            Self::Sid => "SID",
        }
    }

    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Tr808,
            2 => Self::Rock,
            3 => Self::Sid,
            _ => Self::Tr909,
        }
    }
}

impl std::fmt::Display for Kit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ── DrumEvent ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct DrumEvent {
    pub track: Track,
    pub velocity: Velocity,
    pub step: u8,
}

// ── Pattern ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Pattern {
    pub steps: [[Velocity; NUM_STEPS]; NUM_TRACKS],
    pub name: String,
    pub kit: Kit,
    pub tempo: u16,
    pub swing: u8,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            steps: [[Velocity::Off; NUM_STEPS]; NUM_TRACKS],
            name: "New Pattern".to_string(),
            kit: Kit::default(),
            tempo: 120,
            swing: 0,
        }
    }
}

impl Pattern {
    /// Returns all active drum events at the given step, respecting the
    /// CHH/OHH voice-sharing rule: closed hat is silenced when open hat fires.
    pub fn events_at_step(&self, step: u8) -> Vec<DrumEvent> {
        let s = step as usize;
        let ohh_fires = self.steps[Track::OpenHat as usize][s].is_active();

        Track::ALL
            .iter()
            .filter_map(|&track| {
                let vel = self.steps[track as usize][s];
                if !vel.is_active() {
                    return None;
                }
                if track == Track::ClosedHat && ohh_fires {
                    return None;
                }
                Some(DrumEvent {
                    track,
                    velocity: vel,
                    step,
                })
            })
            .collect()
    }
}

// ── Transport ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Transport {
    pub playing: bool,
    pub current_step: u8,
    pub bpm: u16,
    pub swing: u8,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            playing: false,
            current_step: 0,
            bpm: 120,
            swing: 0,
        }
    }
}
