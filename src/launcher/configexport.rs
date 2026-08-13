/// Config / data-directory export and import.
///
/// Zips the entire SumerianClient/ directory (excluding the `accounts/`
/// subdirectory for security) into a portable archive, and restores from one.
use anyhow::{bail, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Export the launcher data dir to a zip, skipping `accounts/`.
/// Returns the path of the created zip.
pub fn export(data_dir: &Path, dest_zip: &PathBuf) -> Result<PathBuf> {
    if dest_zip.exists() {
        bail!("Destination file already exists: {}", dest_zip.display());
    }
    let file = std::fs::File::create(dest_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    add_dir_filtered(&mut zip, data_dir, data_dir, opts, &["accounts"])?;
    zip.finish()?;
    Ok(dest_zip.clone())
}

/// Import / restore a previously exported zip into `data_dir`.
/// Existing files are overwritten; `accounts/` entries in the archive are skipped.
pub fn import(zip_path: &PathBuf, data_dir: &Path) -> Result<usize> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut count = 0usize;

    for i in 0..archive.len() {
        let mut zf = archive.by_index(i)?;
        let name = zf.name().to_string();

        // Skip accounts for security
        if name.starts_with("accounts/") || name.starts_with("accounts\\") {
            continue;
        }

        let out_path = data_dir.join(&name);
        if zf.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(p) = out_path.parent() { std::fs::create_dir_all(p)?; }
            use std::io::Read;
            let mut buf = Vec::new();
            zf.read_to_end(&mut buf)?;
            std::fs::write(&out_path, buf)?;
            count += 1;
        }
    }
    Ok(count)
}

fn add_dir_filtered(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &Path,
    dir: &Path,
    opts: zip::write::FileOptions,
    skip_dirs: &[&str],
) -> Result<()> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        // Skip excluded top-level directories
        if path.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if skip_dirs.iter().any(|s| dir_name.eq_ignore_ascii_case(s)) {
                continue;
            }
            zip.add_directory(&rel_str, opts)?;
            add_dir_filtered(zip, base, &path, opts, skip_dirs)?;
        } else {
            zip.start_file(&rel_str, opts)?;
            let bytes = std::fs::read(&path)?;
            zip.write_all(&bytes)?;
        }
    }
    Ok(())
}
