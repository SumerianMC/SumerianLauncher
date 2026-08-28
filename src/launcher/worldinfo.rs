/// World / seed info reader.
///
/// Extracts basic info from a save's `level.dat` by scanning the raw bytes for
/// known NBT tag signatures — no external NBT crate required.
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct WorldInfo {
    pub name: String,
    /// Seed value (may be unavailable in very old versions).
    pub seed: Option<i64>,
    /// Game mode string: "0"=Survival, "1"=Creative, "2"=Adventure, "3"=Spectator.
    pub game_mode: Option<u8>,
    /// Minecraft data version (numeric).
    pub data_version: Option<i32>,
    /// In-game day count (time / 24000).
    pub day: Option<i64>,
}

impl WorldInfo {
    pub fn game_mode_name(&self) -> &'static str {
        match self.game_mode {
            Some(0) => "Survival",
            Some(1) => "Creative",
            Some(2) => "Adventure",
            Some(3) => "Spectator",
            _ => "Unknown",
        }
    }
}

/// Read world info from a `level.dat` file.
///
/// Uses a heuristic NBT scan rather than a full parser — reliable for all
/// versions from Beta 1.3 onward.
pub fn read(world_dir: &Path) -> WorldInfo {
    let dat_path = world_dir.join("level.dat");
    let compressed = match std::fs::read(&dat_path) {
        Ok(b) => b,
        Err(_) => return WorldInfo { name: world_dir.file_name().unwrap_or_default().to_string_lossy().to_string(), ..Default::default() },
    };

    // level.dat is gzip-compressed.
    let raw = match decompress_gz(&compressed) {
        Some(b) => b,
        None => return WorldInfo { name: world_dir.file_name().unwrap_or_default().to_string_lossy().to_string(), ..Default::default() },
    };

    WorldInfo {
        name: read_string_tag(&raw, b"LevelName").unwrap_or_else(|| {
            world_dir.file_name().unwrap_or_default().to_string_lossy().to_string()
        }),
        seed: read_long_tag(&raw, b"RandomSeed").or_else(|| read_long_tag(&raw, b"WorldGenSettings")),
        game_mode: read_int_tag(&raw, b"GameType").map(|v| v.clamp(0, 3) as u8),
        data_version: read_int_tag(&raw, b"DataVersion"),
        day: read_long_tag(&raw, b"Time").map(|t| t / 24000),
    }
}

// ── Minimal NBT helpers ──────────────────────────────────────────────────────

fn decompress_gz(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

/// Find a TAG_Long (type=4) by its name and return its value.
fn read_long_tag(data: &[u8], name: &[u8]) -> Option<i64> {
    let pos = find_tag(data, 4, name)?;
    if pos + 8 > data.len() { return None; }
    Some(i64::from_be_bytes(data[pos..pos + 8].try_into().ok()?))
}

/// Find a TAG_Int (type=3) by its name and return its value.
fn read_int_tag(data: &[u8], name: &[u8]) -> Option<i32> {
    let pos = find_tag(data, 3, name)?;
    if pos + 4 > data.len() { return None; }
    Some(i32::from_be_bytes(data[pos..pos + 4].try_into().ok()?))
}

/// Find a TAG_String (type=8) by its name and return its value.
fn read_string_tag(data: &[u8], name: &[u8]) -> Option<String> {
    let pos = find_tag(data, 8, name)?;
    if pos + 2 > data.len() { return None; }
    let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + 2 + len > data.len() { return None; }
    String::from_utf8(data[pos + 2..pos + 2 + len].to_vec()).ok()
}

/// Locate the payload start of a named NBT tag with the given type byte.
/// Layout: [type:1][name_len:2][name:n][payload...]
/// Returns the index of the first payload byte.
fn find_tag(data: &[u8], tag_type: u8, name: &[u8]) -> Option<usize> {
    let nl = name.len();
    if nl > 0xFFFF { return None; }
    let needle_len = 1 + 2 + nl;
    if data.len() < needle_len { return None; }

    let mut i = 0;
    while i + needle_len <= data.len() {
        if data[i] == tag_type {
            let n_len = u16::from_be_bytes([data[i + 1], data[i + 2]]) as usize;
            if n_len == nl && &data[i + 3..i + 3 + nl] == name {
                return Some(i + 3 + nl);
            }
        }
        i += 1;
    }
    None
}
