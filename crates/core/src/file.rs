use std::io;
use std::path::Path;

use crate::types::{Kit, Pattern, Velocity, NUM_STEPS, NUM_TRACKS};

/// On-disk layout (current = 149 bytes, v1 = 135 bytes still readable):
///
/// ```text
/// [0..4)        "DB64"  magic
/// [4]           kit     0=909, 1=808, 2=Rock, 3=SID
/// [5]           tempo   BPM (40–250 fits u8; loaded as u16)
/// [6]           swing   0–99
/// [7..23)       name    16 bytes, space-padded
/// [23..135)     steps   7 tracks × 16 steps, each byte = velocity (0–3)
/// [135..142)    volume  7 bytes, 0..=127  (added in v2; defaulted to 100 if absent)
/// [142..149)    pan     7 bytes, signed,  (added in v2; defaulted to 0 if absent)
/// ```
const MAGIC: &[u8; 4] = b"DB64";
const V1_SIZE: usize = 135;
const FILE_SIZE: usize = 149;
const VOL_OFFSET: usize = 135;
const PAN_OFFSET: usize = 142;

impl Pattern {
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let mut buf = [0u8; FILE_SIZE];

        buf[0..4].copy_from_slice(MAGIC);
        buf[4] = self.kit as u8;
        buf[5] = self.tempo.min(255) as u8;
        buf[6] = self.swing.min(99);

        let name = self.name.as_bytes();
        let len = name.len().min(16);
        buf[7..7 + len].copy_from_slice(&name[..len]);
        buf[7 + len..23].fill(b' ');

        for t in 0..NUM_TRACKS {
            for s in 0..NUM_STEPS {
                buf[23 + t * NUM_STEPS + s] = self.steps[t][s] as u8;
            }
            buf[VOL_OFFSET + t] = self.track_volume[t].min(127);
            buf[PAN_OFFSET + t] = self.track_pan[t] as u8; // i8 → u8 round-trip
        }

        std::fs::write(path, &buf)
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let data = std::fs::read(path)?;

        if data.len() < V1_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short"));
        }
        if &data[0..4] != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a DB64 file",
            ));
        }

        let kit = Kit::from_u8(data[4]);
        let tempo = data[5].max(40) as u16;
        let swing = data[6].min(99);
        let name = String::from_utf8_lossy(&data[7..23]).trim_end().to_string();

        let mut steps = [[Velocity::Off; NUM_STEPS]; NUM_TRACKS];
        for t in 0..NUM_TRACKS {
            for s in 0..NUM_STEPS {
                steps[t][s] = Velocity::from_u8(data[23 + t * NUM_STEPS + s]);
            }
        }

        // v2 fields: present only if file is long enough.
        let mut track_volume = [100u8; NUM_TRACKS];
        let mut track_pan = [0i8; NUM_TRACKS];
        if data.len() >= FILE_SIZE {
            for t in 0..NUM_TRACKS {
                track_volume[t] = data[VOL_OFFSET + t].min(127);
                track_pan[t] = data[PAN_OFFSET + t] as i8;
            }
        }

        Ok(Pattern {
            steps,
            name,
            kit,
            tempo,
            swing,
            track_volume,
            track_pan,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Velocity;

    #[test]
    fn round_trip() {
        let mut p = Pattern::default();
        p.name = "Test".to_string();
        p.kit = Kit::Tr808;
        p.tempo = 130;
        p.swing = 42;
        p.steps[0][0] = Velocity::Loud;
        p.steps[1][4] = Velocity::Medium;
        p.steps[6][15] = Velocity::Soft;
        p.track_volume[0] = 110;
        p.track_volume[3] = 64;
        p.track_pan[1] = -32;
        p.track_pan[6] = 60;

        let dir = std::env::temp_dir();
        let path = dir.join("db64_roundtrip.db64");
        p.save(&path).unwrap();

        let loaded = Pattern::load(&path).unwrap();
        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.kit, Kit::Tr808);
        assert_eq!(loaded.tempo, 130);
        assert_eq!(loaded.swing, 42);
        assert_eq!(loaded.steps[0][0], Velocity::Loud);
        assert_eq!(loaded.steps[1][4], Velocity::Medium);
        assert_eq!(loaded.steps[6][15], Velocity::Soft);
        assert_eq!(loaded.track_volume[0], 110);
        assert_eq!(loaded.track_volume[3], 64);
        assert_eq!(loaded.track_pan[1], -32);
        assert_eq!(loaded.track_pan[6], 60);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn loads_v1_file_with_defaults() {
        // Hand-craft a 135-byte v1 file (no vol/pan trailer).
        let mut buf = vec![0u8; V1_SIZE];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4] = Kit::Sid as u8;
        buf[5] = 100;
        buf[6] = 25;
        buf[7..23].fill(b' ');
        buf[7..11].copy_from_slice(b"Old ");
        buf[23] = Velocity::Loud as u8; // track 0 step 0

        let path = std::env::temp_dir().join("db64_v1.db64");
        std::fs::write(&path, &buf).unwrap();

        let loaded = Pattern::load(&path).unwrap();
        assert_eq!(loaded.tempo, 100);
        assert_eq!(loaded.swing, 25);
        assert_eq!(loaded.steps[0][0], Velocity::Loud);
        // Defaults applied
        assert_eq!(loaded.track_volume, [100; NUM_TRACKS]);
        assert_eq!(loaded.track_pan, [0; NUM_TRACKS]);

        std::fs::remove_file(path).ok();
    }
}
