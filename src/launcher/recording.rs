/// Recording helper — detect OBS / replay-mod presence and print setup guides.
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Recorder {
    Obs,
    ReplayMod,
    None,
}

/// Check whether OBS is installed in common locations.
pub fn detect_obs() -> bool {
    // Windows common paths
    let candidates = [
        r"C:\Program Files\obs-studio\bin\64bit\obs64.exe",
        r"C:\Program Files (x86)\obs-studio\bin\32bit\obs32.exe",
        // macOS
        "/Applications/OBS.app/Contents/MacOS/OBS",
        // Linux
        "/usr/bin/obs",
        "/usr/local/bin/obs",
        "/snap/bin/obs-studio",
    ];
    candidates.iter().any(|p| Path::new(p).exists())
}

/// Check if replay-mod jar is present in a mods directory.
pub fn detect_replay_mod(mods_dir: &Path) -> bool {
    std::fs::read_dir(mods_dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.contains("replaymod") && n.ends_with(".jar")
        })
}

/// Detect which recording tool is available.
pub fn detect(mods_dir: &Path) -> Recorder {
    if detect_obs() { return Recorder::Obs; }
    if detect_replay_mod(mods_dir) { return Recorder::ReplayMod; }
    Recorder::None
}

pub fn obs_setup_guide() -> &'static str {
    "OBS Studio — Recording Setup:\n\
     \n\
     1. Open OBS Studio.\n\
     2. In Sources, click '+' → Game Capture (Windows) or Window Capture (macOS/Linux).\n\
     3. Select your Minecraft window.\n\
     4. Set Output → Recording format to MKV (safe if OBS crashes).\n\
     5. Press Start Recording in OBS, then launch Minecraft.\n\
     \n\
     Download: https://obsproject.com"
}

pub fn replay_mod_setup_guide() -> &'static str {
    "ReplayMod — In-game Replay Recording:\n\
     \n\
     1. Install Fabric + ReplayMod for your Minecraft version.\n\
     2. Launch the game — ReplayMod records automatically.\n\
     3. Access replays via the main menu 'Replays' button.\n\
     4. Use the Replay Viewer to render high-quality video clips.\n\
     \n\
     Download: https://www.replaymod.com"
}

pub fn no_recorder_guide() -> &'static str {
    "No recorder detected. Options:\n\
     \n\
     • OBS Studio (any version): https://obsproject.com\n\
       — Free, open-source, records/streams anything.\n\
     \n\
     • ReplayMod (Fabric, 1.8+): https://www.replaymod.com\n\
       — In-game replay system with cinematic camera tools.\n\
     \n\
     • Windows Game Bar (Win 10/11): Win+G while the game is focused.\n\
       — Built-in, no install needed."
}
