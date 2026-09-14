use super::format::{
    ChunkArchive, DecodedSave, MAX_CHUNK_ARCHIVE_BYTES, MAX_SAVE_BYTES, WorldSave, decode,
    decode_chunk_archive, invalid,
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
};

pub(super) fn read_world_save(path: &Path) -> io::Result<DecodedSave> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_SAVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

pub(super) fn read_chunk_archive(path: &Path) -> io::Result<ChunkArchive> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_CHUNK_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    decode_chunk_archive(&bytes)
}

/// Serialize first, fsync a unique temporary sibling, then atomically replace.
pub fn write_world_save(path: &Path, save: &WorldSave) -> io::Result<()> {
    let bytes = save.encode()?;
    atomic_write(path, &bytes)
}

pub(super) fn write_chunk_archive(path: &Path, archive: &ChunkArchive) -> io::Result<()> {
    atomic_write(path, &archive.encode()?)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| invalid("save path must have a parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".world-")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    // Unix directory fsync makes the renamed entry durable as well. Portable
    // std APIs do not provide directory syncing on Windows.
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
