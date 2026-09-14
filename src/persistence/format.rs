use serde::{Deserialize, Serialize};
use std::io;

pub const WORLD_SAVE_VERSION: u32 = 2;
const MAGIC: &[u8; 8] = b"JABASAVE";
pub(super) const MAX_SAVE_BYTES: usize = 64 * 1024;
pub(super) const MAX_CHUNK_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const CHUNK_MAGIC: &[u8; 8] = b"JABACHNK";
pub(super) const CHUNK_ARCHIVE_VERSION: u32 = 1;

/// Stable metadata schema. Changing fields requires a new format version.
/// Primitive arrays keep the disk format independent of Bevy/glam versions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSave {
    pub format_version: u32,
    pub world_name: String,
    pub seed: u64,
    pub game_time_elapsed_days: f64,
    pub player_position: [f32; 3],
    pub player_rotation: PlayerRotation,
}

/// Radians: body yaw around Y and camera pitch around X (no roll).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerRotation {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct ChunkArchive {
    pub version: u32,
    pub seed: u64,
    pub chunks: Vec<SavedChunk>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct SavedChunk {
    pub position: [i32; 3],
    pub blocks: Vec<u16>,
}

pub(super) enum DecodedSave {
    Current(WorldSave),
    LegacyTime(f64),
}

pub(super) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(super) fn valid_world_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

impl WorldSave {
    pub fn validate(&self) -> io::Result<()> {
        if self.format_version != WORLD_SAVE_VERSION {
            return Err(invalid(format!(
                "unsupported save version {}",
                self.format_version
            )));
        }
        if !valid_world_name(&self.world_name) {
            return Err(invalid(
                "world name must be 1–128 ASCII letters, digits, '-' or '_'",
            ));
        }
        if !self.game_time_elapsed_days.is_finite() || self.game_time_elapsed_days < 0.0 {
            return Err(invalid("game time must be finite and non-negative"));
        }
        if self
            .player_position
            .iter()
            .any(|p| !p.is_finite() || p.abs() > 1_000_000.0)
        {
            return Err(invalid("invalid or out-of-range player position"));
        }
        if !self.player_rotation.yaw.is_finite()
            || !self.player_rotation.pitch.is_finite()
            || self.player_rotation.pitch.abs() > std::f32::consts::FRAC_PI_2
        {
            return Err(invalid("invalid player rotation"));
        }
        Ok(())
    }

    pub(super) fn encode(&self) -> io::Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&WORLD_SAVE_VERSION.to_le_bytes());
        bytes.extend(postcard::to_allocvec(self).map_err(|e| invalid(e.to_string()))?);
        Ok(bytes)
    }
}

impl ChunkArchive {
    pub(super) fn encode(&self) -> io::Result<Vec<u8>> {
        if self.version != CHUNK_ARCHIVE_VERSION {
            return Err(invalid("unsupported chunk archive version"));
        }
        let mut bytes = CHUNK_MAGIC.to_vec();
        bytes.extend_from_slice(&CHUNK_ARCHIVE_VERSION.to_le_bytes());
        bytes.extend(postcard::to_allocvec(self).map_err(|e| invalid(e.to_string()))?);
        if bytes.len() > MAX_CHUNK_ARCHIVE_BYTES {
            return Err(invalid("chunk archive exceeds 256 MiB"));
        }
        Ok(bytes)
    }
}

pub(super) fn decode_chunk_archive(bytes: &[u8]) -> io::Result<ChunkArchive> {
    if bytes.len() > MAX_CHUNK_ARCHIVE_BYTES {
        return Err(invalid("chunk archive exceeds 256 MiB"));
    }
    if !bytes.starts_with(CHUNK_MAGIC) {
        return Err(invalid("unknown chunk archive format"));
    }
    let header = bytes
        .get(8..12)
        .ok_or_else(|| invalid("truncated chunk archive header"))?;
    let version = u32::from_le_bytes(header.try_into().expect("four-byte header"));
    if version != CHUNK_ARCHIVE_VERSION {
        return Err(invalid(format!(
            "unsupported chunk archive version {version}"
        )));
    }
    let (archive, trailing): (ChunkArchive, &[u8]) =
        postcard::take_from_bytes(&bytes[12..]).map_err(|e| invalid(e.to_string()))?;
    if !trailing.is_empty() || archive.version != version {
        return Err(invalid("invalid chunk archive payload"));
    }
    Ok(archive)
}

pub(super) fn decode(bytes: &[u8]) -> io::Result<DecodedSave> {
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid("metadata exceeds 64 KiB"));
    }
    if bytes.starts_with(MAGIC) {
        let header = bytes
            .get(8..12)
            .ok_or_else(|| invalid("truncated save header"))?;
        let version = u32::from_le_bytes(header.try_into().expect("four-byte header"));
        if version != WORLD_SAVE_VERSION {
            return Err(invalid(format!("unsupported save version {version}")));
        }
        let (save, trailing): (WorldSave, &[u8]) =
            postcard::take_from_bytes(&bytes[12..]).map_err(|e| invalid(e.to_string()))?;
        if !trailing.is_empty() {
            return Err(invalid("unexpected trailing save data"));
        }
        save.validate()?;
        return Ok(DecodedSave::Current(save));
    }
    // Only the original v1 time-only format is eligible for migration.
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("unknown save format"))?;
    let mut version = None;
    let mut time = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid("invalid legacy save"))?;
        match key.trim() {
            "version" if version.is_none() => version = Some(value.trim()),
            "game_time_elapsed_days" if time.is_none() => {
                time = Some(
                    value
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| invalid("invalid legacy time"))?,
                );
            }
            _ => return Err(invalid("unknown or duplicate legacy field")),
        }
    }
    let time = time
        .filter(|t| t.is_finite() && *t >= 0.0)
        .ok_or_else(|| invalid("missing or invalid legacy time"))?;
    if version != Some("1") {
        return Err(invalid("unsupported legacy save version"));
    }
    Ok(DecodedSave::LegacyTime(time))
}
