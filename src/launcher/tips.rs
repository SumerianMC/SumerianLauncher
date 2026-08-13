/// Startup tip provider.
///
/// Returns a random tip from the bundled list each time the launcher starts.
/// Tips are Minecraft / launcher focused.

static TIPS: &[&str] = &[
    "Use 'Manage Instances → Manage Tags' to group instances by playstyle (e.g. modded, vanilla, speedrun).",
    "Benchmark mode (Settings) samples FPS and heap during your session and shows the result in Playtime.",
    "The conflict detector warns you before launch if two mods are known to be incompatible.",
    "You can assign a session note after every game exit — handy for tracking what you were testing.",
    "The JVM Flag Advisor (Tools menu) tailors GC and heap flags to your Java version and RAM.",
    "Use 'Instance Diff' to see which mods differ between two instances before merging them.",
    "Export a whole instance as a zip to share it with friends or back it up to another machine.",
    "The LAN World Scanner finds open worlds on your local network — great for quick co-op sessions.",
    "Shader presets are version-aware: legacy GLSL for Beta/1.0–1.6, full gbuffers pipeline for 1.7+.",
    "The Friends list TCP-pings your friends' servers so you can see who's online at a glance.",
    "ReplayMod lets you record in-game without any performance hit — check Recording Helper for setup.",
    "You can set per-instance Java binary overrides if you need a specific JDK build for one instance.",
    "The crash pattern learner tracks which mods appear most often in crash reports over time.",
    "Use 'Manage Textures → Resource Pack Wizard' to generate a starter pack with procedural textures.",
    "The Auto optimization profile detects your system RAM and picks the best heap size automatically.",
    "Webhook notifications let you post game start/stop events to Discord, Slack, or any HTTP endpoint.",
    "ZGC (Java 21+) dramatically reduces GC pause stutters — the JVM Advisor recommends it automatically.",
    "You can pin favourite instances to keep them at the top of the list (set a 'pinned' tag).",
    "The world seed reader extracts the seed from any save — no cheats or commands required.",
    "Config export/import lets you move your entire launcher setup to a new machine in one step.",
    "The advancement tracker shows your progress per namespace — great for measuring completion runs.",
    "Use 'Manage Worlds → Seed Info' to view the seed, game mode, and day count of any world.",
    "Trending Mods shows the most-followed mods on Modrinth for your chosen Minecraft version.",
    "The changelog viewer shows release notes before you apply a mod update — no surprises.",
    "Port Forwarding Helper gives you ready-to-run ngrok or playit.gg commands to share your server.",
    "Multiple Microsoft accounts are supported — each stores its token separately and auto-refreshes.",
    "The Auto-backup toggle zips your saves/ before every launch so crashes never lose progress.",
    "You can import any mrpack modpack from Modrinth directly — mods, configs, and overrides included.",
    "Fabric loader versions are fetched live from meta.fabricmc.net — always up to date.",
    "The screenshot gallery lets you copy images to clipboard or upload them to a paste service.",
];

/// Return a pseudo-random tip based on the current minute of the day.
/// Stable within a minute (no rand crate needed), different every minute.
pub fn tip_of_the_session() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mins = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 60)
        .unwrap_or(0) as usize;
    TIPS[mins % TIPS.len()]
}
