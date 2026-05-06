use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

/// Commands sent to the background MIDI-out thread.
#[derive(Debug, Clone)]
pub enum MidiCommand {
    /// One step worth of drum hits: (note, velocity, pan -64..=63).
    Notes(Vec<(u8, u8, i8)>),
    Start,
    Stop,
    Clock,
}

/// Transport / clock messages received from an external MIDI source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiInEvent {
    Clock,
    Start,
    Stop,
    Continue,
}

pub struct MidiBackend {
    conn: MidiOutputConnection,
    channel: u8,
    /// Last CC10 (pan) value sent on this channel; avoids redundant CCs.
    last_pan_cc: Option<u8>,
}

impl MidiBackend {
    /// MIDI channel for GM drums (channel 10 = index 9).
    const DRUM_CHANNEL: u8 = 9;

    pub fn list_ports() -> Vec<String> {
        let Ok(out) = MidiOutput::new("DrumBox64") else {
            return Vec::new();
        };
        let ports = out.ports();
        ports.iter().filter_map(|p| out.port_name(p).ok()).collect()
    }

    pub fn connect(port_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let out = MidiOutput::new("DrumBox64")?;
        let ports = out.ports();
        let port = ports
            .iter()
            .find(|p| out.port_name(p).as_deref() == Ok(port_name))
            .ok_or_else(|| format!("MIDI port not found: {port_name}"))?;
        let conn = out.connect(port, "drumbox64-out")?;
        Ok(Self {
            conn,
            channel: Self::DRUM_CHANNEL,
            last_pan_cc: None,
        })
    }

    /// Dispatch a high-level command.  Called from the background thread.
    pub fn handle(&mut self, cmd: MidiCommand) {
        match cmd {
            MidiCommand::Notes(notes) => {
                for (note, vel, pan) in notes {
                    self.set_pan(pan);
                    self.note_on(note, vel);
                    self.note_off(note);
                }
            }
            MidiCommand::Start => self.send_start(),
            MidiCommand::Stop => {
                self.send_stop();
                self.all_notes_off();
            }
            MidiCommand::Clock => self.send_clock(),
        }
    }

    fn note_on(&mut self, note: u8, vel: u8) {
        let _ = self.conn.send(&[0x90 | self.channel, note, vel]);
    }

    fn note_off(&mut self, note: u8) {
        let _ = self.conn.send(&[0x80 | self.channel, note, 0]);
    }

    /// Send CC10 (Pan) on the drum channel.  Range: pan -64..=63 → 0..=127.
    fn set_pan(&mut self, pan: i8) {
        let cc = (pan as i16 + 64).clamp(0, 127) as u8;
        if self.last_pan_cc == Some(cc) {
            return;
        }
        let _ = self.conn.send(&[0xB0 | self.channel, 10, cc]);
        self.last_pan_cc = Some(cc);
    }

    pub fn all_notes_off(&mut self) {
        let _ = self.conn.send(&[0xB0 | self.channel, 123, 0]);
    }

    pub fn send_start(&mut self) {
        let _ = self.conn.send(&[0xFA]);
    }

    pub fn send_stop(&mut self) {
        let _ = self.conn.send(&[0xFC]);
    }

    pub fn send_clock(&mut self) {
        let _ = self.conn.send(&[0xF8]);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MIDI input — listens for transport / clock messages from an external source.
// ─────────────────────────────────────────────────────────────────────────────

/// Holds an active MIDI input connection.  Drop it to disconnect.
pub struct MidiInBackend {
    _conn: MidiInputConnection<()>,
}

impl MidiInBackend {
    pub fn list_ports() -> Vec<String> {
        let Ok(input) = MidiInput::new("DrumBox64-In") else {
            return Vec::new();
        };
        let ports = input.ports();
        ports
            .iter()
            .filter_map(|p| input.port_name(p).ok())
            .collect()
    }

    /// Connect and forward Start / Stop / Continue / Clock to `callback`.
    /// The callback runs on midir's own thread; keep it short and lock-free.
    pub fn connect<F>(port_name: &str, callback: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn(MidiInEvent) + Send + 'static,
    {
        let input = MidiInput::new("DrumBox64-In")?;
        let ports = input.ports();
        let port = ports
            .iter()
            .find(|p| input.port_name(p).as_deref() == Ok(port_name))
            .ok_or_else(|| format!("MIDI input port not found: {port_name}"))?;
        let conn = input.connect(
            port,
            "drumbox64-in",
            move |_ts, msg, _| {
                if let Some(&byte) = msg.first() {
                    let ev = match byte {
                        0xF8 => Some(MidiInEvent::Clock),
                        0xFA => Some(MidiInEvent::Start),
                        0xFB => Some(MidiInEvent::Continue),
                        0xFC => Some(MidiInEvent::Stop),
                        _ => None,
                    };
                    if let Some(ev) = ev {
                        callback(ev);
                    }
                }
            },
            (),
        )?;
        Ok(Self { _conn: conn })
    }
}
