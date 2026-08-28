/// Custom resource pack creation wizard.
///
/// Generates a valid Minecraft resource pack directory structure inside
/// `textures/packs/<name>/` and optionally writes a small set of procedurally
/// generated block textures.  The PNG files are written as raw RGBA bitmaps
/// encoded into the PNG format without any external image crate — we only need
/// `miniz_oxide` (already pulled in by the `zip` crate) or the `flate2` crate
/// that is already a direct dependency.
use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Metadata provided by the user before generation starts.
pub struct PackSpec {
    pub name: String,
    pub description: String,
    /// Which procedural textures to generate (user can pick any subset).
    pub generate_textures: Vec<ProceduralTexture>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProceduralTexture {
    Grass,
    Stone,
    Dirt,
    Sand,
    Cobblestone,
    Water,
}

impl ProceduralTexture {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Grass,
            Self::Stone,
            Self::Dirt,
            Self::Sand,
            Self::Cobblestone,
            Self::Water,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Grass       => "Grass block (top)",
            Self::Stone       => "Stone",
            Self::Dirt        => "Dirt",
            Self::Sand        => "Sand",
            Self::Cobblestone => "Cobblestone",
            Self::Water       => "Water (still)",
        }
    }

    /// Relative path inside the pack's `assets/minecraft/textures/` directory.
    fn tex_path(&self) -> &'static str {
        match self {
            Self::Grass       => "block/grass_block_top.png",
            Self::Stone       => "block/stone.png",
            Self::Dirt        => "block/dirt.png",
            Self::Sand        => "block/sand.png",
            Self::Cobblestone => "block/cobblestone.png",
            Self::Water       => "block/water_still.png",
        }
    }

    /// Generate a 16×16 RGBA pixel grid for this texture using simple
    /// deterministic noise / patterns — no external image library needed.
    fn generate_pixels(&self) -> Vec<[u8; 4]> {
        let size = 16usize;
        let mut pixels = vec![[0u8; 4]; size * size];
        for y in 0..size {
            for x in 0..size {
                let idx = y * size + x;
                pixels[idx] = self.pixel(x, y, size);
            }
        }
        pixels
    }

    fn pixel(&self, x: usize, y: usize, size: usize) -> [u8; 4] {
        // Simple hash-based noise to give each pixel subtle variation.
        let noise = lcg_noise(x, y) as i32;
        let variation = ((noise % 20) - 10) as i16;
        let v = |base: i16| (base + variation).clamp(0, 255) as u8;

        match self {
            Self::Grass => {
                // Green with lighter centre
                let green: i16 = if (x / 4 + y / 4) % 2 == 0 { 130 } else { 120 };
                [v(55), v(green), v(30), 255]
            }
            Self::Stone => {
                // Grey with subtle banding
                let band: i16 = if (x + y * 2) % 5 == 0 { 120 } else { 140 };
                [v(band), v(band), v(band + 5), 255]
            }
            Self::Dirt => {
                // Brown with small specks
                let speck: i16 = if lcg_noise(x * 3, y * 7) % 8 == 0 { 20 } else { 0 };
                [v(120 + speck), v(80 + speck), v(50 + speck), 255]
            }
            Self::Sand => {
                // Warm beige
                let grain: i16 = if lcg_noise(x * 5, y * 11) % 6 == 0 { 15 } else { 0 };
                [v(210 + grain), v(195 + grain), v(140 + grain), 255]
            }
            Self::Cobblestone => {
                // Dark grey with "grout" lines
                let is_grout = (x % 4 == 0 && y % 4 < 2) || (x % 4 < 2 && y % 8 == 0);
                if is_grout {
                    [80, 80, 80, 255]
                } else {
                    let c: i16 = if (x / 4 + y / 4) % 2 == 0 { 130 } else { 115 };
                    [v(c), v(c), v(c), 255]
                }
            }
            Self::Water => {
                // Translucent blue with animated-style waves
                let wave: i16 = if (x + y + size) % 3 == 0 { 20 } else { 0 };
                [v(30 + wave), v(80 + wave), v(200 + wave), 180]
            }
        }
    }
}

/// Tiny deterministic pseudo-random function — no rand crate needed.
fn lcg_noise(x: usize, y: usize) -> usize {
    let v = x.wrapping_mul(1664525).wrapping_add(y.wrapping_mul(1013904223));
    v ^ (v >> 11)
}

/// Build the resource pack at `base_packs_dir/<spec.name>/`.
pub async fn create_pack(base_packs_dir: &Path, spec: &PackSpec) -> Result<PathBuf> {
    let pack_dir = base_packs_dir.join(&spec.name);
    if pack_dir.exists() {
        anyhow::bail!("A pack named '{}' already exists.", spec.name);
    }

    // ── pack.mcmeta ──────────────────────────────────────────────────────────
    let textures_block_dir = pack_dir
        .join("assets")
        .join("minecraft")
        .join("textures")
        .join("block");
    fs::create_dir_all(&textures_block_dir).await?;

    let mcmeta = serde_json::json!({
        "pack": {
            "pack_format": 34,   // 1.21.x
            "description": spec.description
        }
    });
    let mcmeta_path = pack_dir.join("pack.mcmeta");
    fs::write(&mcmeta_path, serde_json::to_string_pretty(&mcmeta)?).await?;

    // ── Procedural textures ──────────────────────────────────────────────────
    for tex in &spec.generate_textures {
        let pixels = tex.generate_pixels();
        let png_bytes = encode_png_16x16(&pixels)
            .with_context(|| format!("Failed to encode PNG for {}", tex.display_name()))?;
        let rel = tex.tex_path();
        let out = pack_dir.join("assets").join("minecraft").join("textures").join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&out, &png_bytes).await?;
    }

    Ok(pack_dir)
}

// ── Minimal PNG encoder (no external image crate) ────────────────────────────
//
// We only need to encode a 16×16 RGBA image.  The PNG format is:
//   8-byte signature
//   IHDR chunk  (13 bytes of data)
//   IDAT chunk  (deflate-compressed scanlines)
//   IEND chunk
//
// We use the `flate2` crate (already in Cargo.toml) for deflate.

fn encode_png_16x16(pixels: &[[u8; 4]]) -> Result<Vec<u8>> {
    use flate2::{write::ZlibEncoder, Compression};

    const W: u32 = 16;
    const H: u32 = 16;

    // Build raw scanlines (filter byte 0 = None, then RGBA row).
    let mut raw = Vec::with_capacity(((W * 4 + 1) * H) as usize);
    for row in 0..H as usize {
        raw.push(0u8); // filter type = None
        for col in 0..W as usize {
            let p = &pixels[row * W as usize + col];
            raw.extend_from_slice(p);
        }
    }

    // Compress with zlib.
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw)?;
    let compressed = encoder.finish()?;

    // Assemble PNG.
    let mut out: Vec<u8> = Vec::new();

    // PNG signature.
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    // IHDR chunk.
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&W.to_be_bytes());
    ihdr_data.extend_from_slice(&H.to_be_bytes());
    ihdr_data.push(8);  // bit depth
    ihdr_data.push(6);  // colour type: RGBA
    ihdr_data.extend_from_slice(&[0, 0, 0]); // compression, filter, interlace
    write_chunk(&mut out, b"IHDR", &ihdr_data);

    // IDAT chunk.
    write_chunk(&mut out, b"IDAT", &compressed);

    // IEND chunk.
    write_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    // CRC over chunk_type + data.
    let crc = crc32(chunk_type, data);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// CRC-32 (ISO 3309) — needed for valid PNG chunks.
fn crc32(type_bytes: &[u8], data: &[u8]) -> u32 {
    // Standard CRC-32 table lookup.
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for n in 0..256u32 {
            let mut c = n;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
            }
            t[n as usize] = c;
        }
        t
    });

    let mut crc: u32 = 0xFFFFFFFF;
    for &b in type_bytes.iter().chain(data.iter()) {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}
