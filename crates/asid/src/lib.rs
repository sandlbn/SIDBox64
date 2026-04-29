pub mod kits;

use drumbox64_core::{DrumEvent, Kit, Velocity};
use kits::{DrumVoice, TRACK_TO_VOICE, VOICE_NUM, VOICE_SID};

/// Active voice sweep state (mirrors VoiceState in original drumbox.h).
#[derive(Debug, Default, Clone)]
struct VoiceState {
    freq: u16,
    freq_end: u16,
    sweep: u16,
    ttl: u8,
    active: bool,
    wave2: u8,
    wave2_tick: u8,
    freq2: u16,
}

/// ASID/USBSID-Pico backend.
///
/// With the `hardware` feature: connects to a real USBSID-Pico over USB,
/// uses the threaded ring buffer for low-latency writes, and maps dual-SID
/// registers via the 0x20 stride (SID2 = SID1 reg + 0x20).
///
/// Without `hardware`: all register writes are no-ops; voice state is still
/// tracked so the struct can be used in tests.
pub struct AsidBackend {
    voices: [VoiceState; 6],
    kit: Kit,

    #[cfg(feature = "hardware")]
    device: usbsid_pico::UsbSid,
}

impl AsidBackend {
    /// Create a no-op backend (non-hardware builds only).
    #[cfg(not(feature = "hardware"))]
    pub fn new(kit: Kit) -> Self {
        Self {
            voices: Default::default(),
            kit,
        }
    }

    /// Try to open the USBSID-Pico and return a connected backend.
    ///
    /// Uses threaded + cycled mode so register writes are non-blocking.
    /// Returns `Err` on failure or when compiled without the `hardware` feature.
    pub fn try_connect(kit: Kit) -> Result<Self, String> {
        #[cfg(feature = "hardware")]
        {
            let mut device = usbsid_pico::UsbSid::new();
            device
                .init(true, true)
                .map_err(|e| format!("USBSID-Pico: {e}"))?;
            device.reset();
            device.unmute();
            return Ok(Self {
                voices: Default::default(),
                kit,
                device,
            });
        }
        #[cfg(not(feature = "hardware"))]
        {
            let _ = kit;
            Err(
                "ASID hardware support not compiled in (rebuild with --features hardware)"
                    .to_string(),
            )
        }
    }

    pub fn set_kit(&mut self, kit: Kit) {
        self.kit = kit;
    }

    /// Trigger a drum event, programming the SID voice registers.
    /// `volume_127` scales the SID master-volume derived from the velocity (127 = no scaling).
    pub fn trigger(&mut self, event: &DrumEvent, volume_127: u8) {
        if event.velocity == Velocity::Off {
            return;
        }

        let track_idx = event.track as usize;
        let vi = TRACK_TO_VOICE[track_idx];
        let sid_chip = VOICE_SID[vi];
        let voice = VOICE_NUM[vi];
        let kv = &kits::kit_for(self.kit)[track_idx];
        let base_vol = event.velocity.sid_vol() as u16;
        let vol = ((base_vol * volume_127 as u16 + 64) / 127).min(15) as u8;

        self.sid_write_voice(sid_chip, voice, kv, vol);

        let vs = &mut self.voices[vi];
        vs.freq = kv.freq_start;
        vs.freq_end = kv.freq_end;
        vs.sweep = kv.sweep_rate;
        vs.ttl = kv.ttl;
        vs.active = true;
        vs.wave2 = kv.wave2;
        vs.wave2_tick = kv.wave2_tick;
        vs.freq2 = kv.freq2;

        self.flush();
    }

    /// Advance sweep state by one tick (~240 Hz).  Call from a timer callback.
    pub fn tick(&mut self) {
        struct RegWrite {
            sid: u8,
            reg: u8,
            val: u8,
        }
        let mut writes: Vec<RegWrite> = Vec::new();

        for vi in 0..6usize {
            if !self.voices[vi].active {
                continue;
            }
            let sid_chip = VOICE_SID[vi];
            let voice = VOICE_NUM[vi];

            // Waveform switch (Hubbard technique)
            if self.voices[vi].wave2 != 0 && self.voices[vi].wave2_tick > 0 {
                self.voices[vi].wave2_tick -= 1;
                if self.voices[vi].wave2_tick == 0 {
                    if self.voices[vi].freq2 != 0 {
                        let f = self.voices[vi].freq2;
                        writes.push(RegWrite {
                            sid: sid_chip,
                            reg: voice * 7,
                            val: (f & 0xFF) as u8,
                        });
                        writes.push(RegWrite {
                            sid: sid_chip,
                            reg: voice * 7 + 1,
                            val: (f >> 8) as u8,
                        });
                        self.voices[vi].freq = f;
                    }
                    let w2 = self.voices[vi].wave2;
                    writes.push(RegWrite {
                        sid: sid_chip,
                        reg: voice * 7 + 4,
                        val: w2,
                    });
                    self.voices[vi].wave2 = 0;
                }
            }

            // Gate duration
            if self.voices[vi].ttl > 0 {
                self.voices[vi].ttl -= 1;
                if self.voices[vi].ttl == 0 {
                    writes.push(RegWrite {
                        sid: sid_chip,
                        reg: voice * 7 + 4,
                        val: 0,
                    });
                    self.voices[vi].active = false;
                    continue;
                }
            }

            // Pitch sweep
            if self.voices[vi].sweep != 0 && self.voices[vi].freq > self.voices[vi].freq_end {
                let next =
                    if self.voices[vi].freq > self.voices[vi].freq_end + self.voices[vi].sweep {
                        self.voices[vi].freq - self.voices[vi].sweep
                    } else {
                        self.voices[vi].freq_end
                    };
                self.voices[vi].freq = next;
                writes.push(RegWrite {
                    sid: sid_chip,
                    reg: voice * 7,
                    val: (next & 0xFF) as u8,
                });
                writes.push(RegWrite {
                    sid: sid_chip,
                    reg: voice * 7 + 1,
                    val: (next >> 8) as u8,
                });
            }
        }

        for w in writes {
            self.write_reg(w.sid, w.reg, w.val);
        }
        self.flush();
    }

    /// Mute all SID chips.
    pub fn mute(&mut self) {
        #[cfg(feature = "hardware")]
        self.device.mute();
    }

    /// Reset all SID chips and clear voice state.
    pub fn reset(&mut self) {
        #[cfg(feature = "hardware")]
        self.device.reset();
        self.voices = Default::default();
    }

    fn sid_write_voice(&mut self, sid: u8, voice: u8, kv: &DrumVoice, vol: u8) {
        let base = voice * 7;
        self.write_reg(sid, 0x18, vol);
        if (kv.waveform & kits::PULSE != 0) || (kv.wave2 & kits::PULSE != 0) {
            self.write_reg(sid, base + 2, 0x00);
            self.write_reg(sid, base + 3, 0x08);
        }
        self.write_reg(sid, base + 4, kits::GATE << 3); // TEST bit — reset oscillator
        self.write_reg(sid, base + 5, kv.attack_decay);
        self.write_reg(sid, base + 6, kv.sustain_release);
        self.write_reg(sid, base, (kv.freq_start & 0xFF) as u8);
        self.write_reg(sid, base + 1, (kv.freq_start >> 8) as u8);
        self.write_reg(sid, base + 4, kv.waveform | kits::GATE);
    }

    fn write_reg(&mut self, sid: u8, reg: u8, val: u8) {
        #[cfg(feature = "hardware")]
        {
            // SID2 registers are offset by 0x20 from SID1 in USBSID-Pico addressing.
            let global_reg = reg + sid * 0x20;
            let _ = self.device.write_ring_cycled(global_reg, val, 8);
        }
        #[cfg(not(feature = "hardware"))]
        {
            let _ = (sid, reg, val);
        }
    }

    fn flush(&mut self) {
        #[cfg(feature = "hardware")]
        self.device.set_flush();
    }
}
