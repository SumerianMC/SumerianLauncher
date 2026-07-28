use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::optimizer::OptimizationProfile;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstanceProfile {
    #[serde(default)]
    pub optimization: Option<OptimizationProfile>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub custom_jvm_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub name: String,
    pub version_id: String,
    pub created_at: String,
    #[serde(default)]
    pub notes: String,
}

/// Per-instance mod enable/disable profile stored as `mod_profile.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModProfile {
    /// Filenames of mods that are disabled (renamed to .jar.disabled on disk).
    #[serde(default)]
    pub disabled: Vec<String>,
}

pub struct InstanceManager {
    instances_dir: PathBuf,
    index_path: PathBuf,
}

impl InstanceManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            instances_dir: data_dir.join("instances"),
            index_path: data_dir.join("instances.json"),
        }
    }

    pub async fn load_all(&self) -> Result<Vec<Instance>> {
        match fs::read_to_string(&self.index_path).await {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    async fn save_all(&self, instances: &[Instance]) -> Result<()> {
        fs::create_dir_all(&self.instances_dir).await?;
        fs::write(&self.index_path, serde_json::to_string_pretty(instances)?).await?;
        Ok(())
    }

    pub async fn create(&self, name: &str, version_id: &str) -> Result<Instance> {
        let mut all = self.load_all().await?;
        if all.iter().any(|i| i.name.eq_ignore_ascii_case(name)) {
            bail!("Instance '{}' already exists.", name);
        }
        let dir = self.instance_dir(name);
        for sub in &["mods", "saves", "resourcepacks", "shaderpacks", "config"] {
            fs::create_dir_all(dir.join(sub)).await?;
        }
        let instance = Instance {
            name: name.to_string(),
            version_id: version_id.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            notes: String::new(),
        };
        all.push(instance.clone());
        self.save_all(&all).await?;
        Ok(instance)
    }

    pub async fn set_notes(&self, name: &str, notes: &str) -> Result<()> {
        let mut all = self.load_all().await?;
        match all.iter_mut().find(|i| i.name.eq_ignore_ascii_case(name)) {
            Some(i) => i.notes = notes.to_string(),
            None => bail!("Instance '{}' not found.", name),
        }
        self.save_all(&all).await
    }

    pub async fn clone_instance(&self, src: &str, new_name: &str) -> Result<Instance> {
        let mut all = self.load_all().await?;
        if all.iter().any(|i| i.name.eq_ignore_ascii_case(new_name)) {
            bail!("Instance '{}' already exists.", new_name);
        }
        let src_inst = all.iter().find(|i| i.name.eq_ignore_ascii_case(src))
            .ok_or_else(|| anyhow::anyhow!("Instance '{}' not found.", src))?
            .clone();
        let src_dir = self.instance_dir(src);
        let dst_dir = self.instance_dir(new_name);
        copy_dir_all(&src_dir, &dst_dir).await?;
        let new_inst = Instance {
            name: new_name.to_string(),
            version_id: src_inst.version_id.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            notes: src_inst.notes.clone(),
        };
        all.push(new_inst.clone());
        self.save_all(&all).await?;
        Ok(new_inst)
    }

    pub async fn delete(&self, name: &str) -> Result<()> {
        let mut all = self.load_all().await?;
        let before = all.len();
        all.retain(|i| !i.name.eq_ignore_ascii_case(name));
        if all.len() == before {
            bail!("Instance '{}' not found.", name);
        }
        let dir = self.instance_dir(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).await?;
        }
        self.save_all(&all).await
    }

    pub fn instance_dir(&self, name: &str) -> PathBuf {
        self.instances_dir.join(name)
    }

    fn profile_path(&self, name: &str) -> PathBuf {
        self.instance_dir(name).join("instance_profile.json")
    }

    pub async fn load_profile(&self, name: &str) -> InstanceProfile {
        let path = self.profile_path(name);
        match fs::read_to_string(&path).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => InstanceProfile::default(),
        }
    }

    pub async fn save_profile(&self, name: &str, profile: &InstanceProfile) -> Result<()> {
        let path = self.profile_path(name);
        fs::write(path, serde_json::to_string_pretty(profile)?).await?;
        Ok(())
    }

    fn mod_profile_path(&self, name: &str) -> PathBuf {
        self.instance_dir(name).join("mod_profile.json")
    }

    pub async fn load_mod_profile(&self, name: &str) -> ModProfile {
        match fs::read_to_string(self.mod_profile_path(name)).await {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => ModProfile::default(),
        }
    }

    pub async fn save_mod_profile(&self, name: &str, mp: &ModProfile) -> Result<()> {
        fs::write(self.mod_profile_path(name), serde_json::to_string_pretty(mp)?).await?;
        Ok(())
    }

    /// Disable a mod by renaming `name.jar` → `name.jar.disabled`.
    pub async fn disable_mod(&self, instance: &str, filename: &str) -> Result<()> {
        let mods_dir = self.instance_dir(instance).join("mods");
        let src = mods_dir.join(filename);
        let dst = mods_dir.join(format!("{}.disabled", filename));
        if !src.exists() { bail!("Mod '{}' not found.", filename); }
        fs::rename(src, dst).await?;
        let mut mp = self.load_mod_profile(instance).await;
        if !mp.disabled.contains(&filename.to_string()) {
            mp.disabled.push(filename.to_string());
        }
        self.save_mod_profile(instance, &mp).await
    }

    /// Re-enable a mod by renaming `name.jar.disabled` → `name.jar`.
    pub async fn enable_mod(&self, instance: &str, filename: &str) -> Result<()> {
        let mods_dir = self.instance_dir(instance).join("mods");
        let src = mods_dir.join(format!("{}.disabled", filename));
        let dst = mods_dir.join(filename);
        if !src.exists() { bail!("Disabled mod '{}' not found.", filename); }
        fs::rename(src, dst).await?;
        let mut mp = self.load_mod_profile(instance).await;
        mp.disabled.retain(|f| f != filename);
        self.save_mod_profile(instance, &mp).await
    }

    pub async fn export(&self, name: &str, dest_path: &PathBuf) -> Result<()> {
        let inst_dir = self.instance_dir(name);
        if !inst_dir.exists() {
            bail!("Instance directory not found.");
        }
        let instances = self.load_all().await?;
        let inst = instances.iter().find(|i| i.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow::anyhow!("Instance '{}' not found", name))?;
        let manifest = serde_json::to_string_pretty(inst)?;

        let file = std::fs::File::create(dest_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("instance.json", opts)?;
        use std::io::Write;
        zip.write_all(manifest.as_bytes())?;

        add_dir_to_zip(&mut zip, &inst_dir, &inst_dir, opts)?;
        zip.finish()?;
        Ok(())
    }

    pub async fn import(&self, zip_path: &PathBuf) -> Result<Instance> {
        let file = std::fs::File::open(zip_path)?;
        let mut zip = zip::ZipArchive::new(file)?;

        let inst: Instance = {
            let mut mf = zip.by_name("instance.json")
                .map_err(|_| anyhow::anyhow!("Not a valid Sumerian instance export (missing instance.json)"))?;
            use std::io::Read;
            let mut s = String::new();
            mf.read_to_string(&mut s)?;
            serde_json::from_str(&s)?
        };

        let mut all = self.load_all().await?;
        if all.iter().any(|i| i.name.eq_ignore_ascii_case(&inst.name)) {
            bail!("Instance '{}' already exists. Delete it first.", inst.name);
        }

        let dest_dir = self.instance_dir(&inst.name);
        std::fs::create_dir_all(&dest_dir)?;

        for i in 0..zip.len() {
            let mut zf = zip.by_index(i)?;
            if zf.name() == "instance.json" { continue; }
            let out_path = dest_dir.join(zf.name());
            if zf.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(p) = out_path.parent() { std::fs::create_dir_all(p)?; }
                use std::io::Read;
                let mut buf = Vec::new();
                zf.read_to_end(&mut buf)?;
                std::fs::write(&out_path, buf)?;
            }
        }

        all.push(inst.clone());
        self.save_all(&all).await?;
        Ok(inst)
    }
}

#[derive(Debug, Clone)]
pub struct WorldInfo {
    pub name: String,
    pub path: PathBuf,
    pub last_played: Option<String>,
}

pub struct WorldManager;

impl WorldManager {
    pub async fn list(saves_dir: &PathBuf) -> Result<Vec<WorldInfo>> {
        let mut worlds = Vec::new();
        let Ok(mut dir) = fs::read_dir(saves_dir).await else {
            return Ok(worlds);
        };
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            // Read level.dat modified time as last_played
            let last_played = std::fs::metadata(path.join("level.dat"))
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.format("%Y-%m-%d %H:%M").to_string()
                });
            worlds.push(WorldInfo { name, path, last_played });
        }
        worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        Ok(worlds)
    }

    pub async fn rename(saves_dir: &PathBuf, old_name: &str, new_name: &str) -> Result<()> {
        let old = saves_dir.join(old_name);
        let new = saves_dir.join(new_name);
        if !old.exists() { bail!("World '{}' not found.", old_name); }
        if new.exists() { bail!("A world named '{}' already exists.", new_name); }
        fs::rename(old, new).await?;
        Ok(())
    }

    pub async fn delete(saves_dir: &PathBuf, name: &str) -> Result<()> {
        let path = saves_dir.join(name);
        if !path.exists() { bail!("World '{}' not found.", name); }
        fs::remove_dir_all(path).await?;
        Ok(())
    }

    pub async fn export(saves_dir: &PathBuf, name: &str, dest: &PathBuf) -> Result<()> {
        let world_dir = saves_dir.join(name);
        if !world_dir.exists() { bail!("World '{}' not found.", name); }
        let file = std::fs::File::create(dest)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        add_dir_to_zip(&mut zip, &world_dir, &world_dir, opts)?;
        zip.finish()?;
        Ok(())
    }
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<std::fs::File>,
    base: &PathBuf,
    dir: &PathBuf,
    opts: zip::write::FileOptions,
) -> Result<()> {
    use std::io::Write;
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zip.add_directory(&rel_str, opts)?;
            add_dir_to_zip(zip, base, &path, opts)?;
        } else {
            zip.start_file(&rel_str, opts)?;
            let bytes = std::fs::read(&path)?;
            zip.write_all(&bytes)?;
        }
    }
    Ok(())
}

async fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    fs::create_dir_all(dst).await?;
    let mut rd = fs::read_dir(src).await?;
    while let Some(entry) = rd.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
        } else {
            fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}
