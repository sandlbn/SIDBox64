use std::io;
use std::path::Path;

use crate::types::{Kit, Pattern, Velocity, NUM_STEPS, NUM_TRACKS};

/// On-disk layout (135 bytes, backwards-compatible with C64 original):
///
/// ```text
/// [0..4)   "DB64"  magic
/// [4]      kit     0=909, 1=808, 2=Rock, 3=SID
/// [5]      tempo   BPM (40–250 fits u8; stored as u8, loaded as u16)
/// [6]      swing   0–99
/// [7..23)  name    16 bytes, space-padded
/// [23..135) steps  7 tracks × 16 steps, each byte = velocity (0–3)
/// ```
const MAGIC: &[u8; 4] = b"DB64";
const FILE_SIZE: usize = 135;

impl Pattern {
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let mut buf = [0u8; FILE_SIZE];

        buf[0..4].copy_from_slice(MAGIC);
        buf[4] = self.kit as u8;
        buf[5] = self.tempo.min(255) as u8;
        buf[6] = self.swing.min(99);

        // Name: space-padded to 16 bytes
        let name = self.name.as_bytes();
        let len = name.len().min(16);
        buf[7..7 + len].copy_from_slice(&name[..len]);
        buf[7 + len..23].fill(b' ');

        // Steps
        for t in 0..NUM_TRACKS {
            for s in 0..NUM_STEPS {
                buf[23 + t * NUM_STEPS + s] = self.steps[t][s] as u8;
            }
        }

        std::fs::write(path, &buf)
    }

    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let data = std::fs::read(path)?;

        if data.len() < FILE_SIZE {
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

        Ok(Pattern {
            steps,
            name,
            kit,
            tempo,
            swing,
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

        let dir = std::env::temp_dir();
        let path = dir.join("db64_test.db64");
        p.save(&path).unwrap();

        let loaded = Pattern::load(&path).unwrap();
        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.kit, Kit::Tr808);
        assert_eq!(loaded.tempo, 130);
        assert_eq!(loaded.swing, 42);
        assert_eq!(loaded.steps[0][0], Velocity::Loud);
        assert_eq!(loaded.steps[1][4], Velocity::Medium);
        assert_eq!(loaded.steps[6][15], Velocity::Soft);

        std::fs::remove_file(path).ok();
    }
}
