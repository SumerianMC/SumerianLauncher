use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct ScreenshotGallery;

impl ScreenshotGallery {
    /// List all PNG screenshots in the given game/instance dir's screenshots folder.
    pub async fn list(game_dir: &Path) -> Result<Vec<PathBuf>> {
        let dir = game_dir.join("screenshots");
        let mut shots = Vec::new();
        let mut rd = match fs::read_dir(&dir).await {
            Ok(d) => d,
            Err(_) => return Ok(shots),
        };
        while let Some(entry) = rd.next_entry().await? {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("png") {
                shots.push(p);
            }
        }
        // Sort newest first by filename (Minecraft names them by date)
        shots.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        Ok(shots)
    }

    /// Open a screenshot with the system default image viewer.
    pub fn open(path: &PathBuf) -> anyhow::Result<()> {
        open::that(path)?;
        Ok(())
    }

    /// Open the screenshots folder in the system file explorer.
    pub fn open_folder(game_dir: &Path) -> anyhow::Result<()> {
        open::that(game_dir.join("screenshots"))?;
        Ok(())
    }

    /// Copy the file path to the system clipboard using a platform command.
    /// Returns Ok(true) if successful, Ok(false) if no clipboard tool found.
    pub fn copy_path_to_clipboard(path: &Path) -> anyhow::Result<bool> {
        let path_str = path.to_string_lossy().to_string();

        // Windows: clip command
        #[cfg(target_os = "windows")]
        {
            let status = std::process::Command::new("cmd")
                .args(["/C", &format!("echo {}| clip", path_str)])
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                return Ok(true);
            }
        }

        // macOS: pbcopy
        #[cfg(target_os = "macos")]
        {
            use std::io::Write;
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(path_str.as_bytes())?;
            }
            child.wait()?;
            return Ok(true);
        }

        // Linux: xclip or xsel
        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            for cmd in &["xclip", "xsel"] {
                let args: &[&str] = if *cmd == "xclip" {
                    &["-selection", "clipboard"]
                } else {
                    &["--clipboard", "--input"]
                };
                if let Ok(mut child) = std::process::Command::new(cmd)
                    .args(args)
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                {
                    if let Some(stdin) = child.stdin.as_mut() {
                        let _ = stdin.write_all(path_str.as_bytes());
                    }
                    let _ = child.wait();
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Upload a screenshot to 0x0.st and return the public URL.
    pub async fn upload_to_paste(
        client: &reqwest::Client,
        path: &PathBuf,
    ) -> anyhow::Result<String> {
        let bytes = fs::read(path).await?;
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "screenshot.png".into());

        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename)
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new().part("file", part);

        let resp = client
            .post("https://0x0.st")
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Upload failed with status {}", resp.status());
        }

        let url = resp.text().await?.trim().to_string();
        Ok(url)
    }
}
