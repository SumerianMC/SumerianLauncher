/// World Seed Reader — extract the world seed from a Minecraft `level.dat`
/// file without launching the game.
///
/// `level.dat` is a gzip-compressed NBT file.  The seed is stored at:
///   `Data > WorldGenSettings > seed`  (1.16+)
///   `Data > RandomSeed`               (pre-1.16)
///
/// This parser is intentionally minimal — it only needs to locate a single
/// `TAG_Long` value, so it avoids pulling in a full NBT library.
use anyhow::{bail, Result};
use std::io::{BufRead, Read};
use std::path::Path;

/// Read the world seed from `<world_dir>/level.dat`.
/// Returns the seed as a signed 64-bit integer.
pub fn read_seed(world_dir: &Path) -> Result<i64> {
    let level_dat = world_dir.join("level.dat");
    if !level_dat.exists() {
        bail!("level.dat not found in {}", world_dir.display());
    }

    let raw = std::fs::read(&level_dat)?;
    // level.dat is always gzip-compressed.
    let mut decoder = flate2::read::GzDecoder::new(raw.as_slice());
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;

    // Try modern path first (1.16+ WorldGenSettings.seed), then legacy RandomSeed.
    if let Some(seed) = find_long_tag(&buf, b"seed") {
        return Ok(seed);
    }
    if let Some(seed) = find_long_tag(&buf, b"RandomSeed") {
        return Ok(seed);
    }

    bail!("Could not find seed in level.dat (unsupported format)");
}

/// Scan raw NBT bytes for a TAG_Long (type 4) with the given name.
/// Returns the first match.
///
/// NBT layout for a named tag:
///   [1 byte]  tag type
///   [2 bytes] name length (big-endian u16)
///   [N bytes] name
///   [payload] (type-dependent)
fn find_long_tag(data: &[u8], name: &[u8]) -> Option<i64> {
    const TAG_LONG: u8 = 4;
    let name_len = name.len();
    if data.len() < 3 + name_len + 8 {
        return None;
    }

    let mut i = 0usize;
    while i + 3 + name_len + 8 <= data.len() {
        if data[i] == TAG_LONG {
            let stored_len = u16::from_be_bytes([data[i + 1], data[i + 2]]) as usize;
            if stored_len == name_len {
                let tag_name_start = i + 3;
                let tag_name_end   = tag_name_start + name_len;
                if tag_name_end + 8 <= data.len()
                    && &data[tag_name_start..tag_name_end] == name
                {
                    let payload = &data[tag_name_end..tag_name_end + 8];
                    let seed = i64::from_be_bytes(payload.try_into().ok()?);
                    return Some(seed);
                }
            }
        }
        i += 1;
    }
    None
}

/// Format a seed for display — negative seeds are common and valid.
pub fn format_seed(seed: i64) -> String {
    seed.to_string()
}
