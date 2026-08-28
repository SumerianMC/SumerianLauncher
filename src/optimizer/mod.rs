use serde::{Deserialize, Serialize};
use std::fmt;

/// Detect total system RAM in MB using sysinfo.
pub fn system_ram_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

/// Suggest the best OptimizationProfile for the current system RAM.
pub fn auto_tune() -> OptimizationProfile {
    match system_ram_mb() {
        0..=2047   => OptimizationProfile::Potato,
        2048..=5119 => OptimizationProfile::Balanced,
        5120..=9215 => OptimizationProfile::Performance,
        _           => OptimizationProfile::Quality,
    }
}

/// Return a -Xmx value (in MB) that is 50% of system RAM, capped at 8192.
pub fn auto_heap_mb() -> u64 {
    (system_ram_mb() / 2).min(8192)
}

/// Build fully-tuned JVM flags based on detected system RAM.
/// Heap is set to 50 % of RAM (capped at 8192 MB); GC flags match the
/// profile that `auto_tune()` would pick.
pub fn auto_tune_flags() -> Vec<String> {
    let heap_mb = auto_heap_mb();
    let xmx = format!("-Xmx{}M", heap_mb);
    let xms = format!("-Xms{}M", (heap_mb / 8).max(256));
    let profile = auto_tune();
    // Take the chosen profile's flags but replace its hardcoded -Xmx/-Xms
    let mut flags: Vec<String> = profile
        .jvm_flags()
        .into_iter()
        .filter(|f| !f.starts_with("-Xmx") && !f.starts_with("-Xms"))
        .collect();
    flags.insert(0, xms);
    flags.insert(0, xmx);
    flags
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum OptimizationProfile {
    Auto,
    Performance,
    #[default]
    Balanced,
    Quality,
    Potato,
}

impl fmt::Display for OptimizationProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto"),
            Self::Performance => write!(f, "Performance"),
            Self::Balanced => write!(f, "Balanced"),
            Self::Quality => write!(f, "Quality"),
            Self::Potato => write!(f, "Potato"),
        }
    }
}

impl OptimizationProfile {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Auto,
            Self::Performance,
            Self::Balanced,
            Self::Quality,
            Self::Potato,
        ]
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Auto,
            1 => Self::Performance,
            2 => Self::Balanced,
            3 => Self::Quality,
            _ => Self::Potato,
        }
    }

    /// Returns JVM flags for this profile.
    pub fn jvm_flags(&self) -> Vec<String> {
        match self {
            Self::Auto => crate::optimizer::auto_tune_flags(),
            Self::Performance => vec![
                "-Xmx4G".into(),
                "-Xms512M".into(),
                "-XX:+UseG1GC".into(),
                "-XX:G1HeapRegionSize=32M".into(),
                "-XX:MaxGCPauseMillis=50".into(),
                "-XX:+UnlockExperimentalVMOptions".into(),
                "-XX:+DisableExplicitGC".into(),
                "-XX:+AlwaysPreTouch".into(),
                "-XX:+ParallelRefProcEnabled".into(),
            ],
            Self::Balanced => vec![
                "-Xmx2G".into(),
                "-Xms256M".into(),
                "-XX:+UseG1GC".into(),
                "-XX:MaxGCPauseMillis=100".into(),
                "-XX:+UnlockExperimentalVMOptions".into(),
            ],
            Self::Quality => vec![
                "-Xmx6G".into(),
                "-Xms1G".into(),
                "-XX:+UseG1GC".into(),
                "-XX:G1HeapRegionSize=64M".into(),
                "-XX:MaxGCPauseMillis=200".into(),
                "-XX:+UnlockExperimentalVMOptions".into(),
                "-XX:+AlwaysPreTouch".into(),
            ],
            Self::Potato => vec![
                "-Xmx512M".into(),
                "-Xms128M".into(),
                "-XX:+UseSerialGC".into(),
                "-XX:+OptimizeStringConcat".into(),
            ],
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Auto => {
                let mb = crate::optimizer::auto_heap_mb();
                let profile = crate::optimizer::auto_tune();
                // Return a static-lifetime str isn't possible with runtime data,
                // so we use a fixed label here; the Select prompt shows RAM detail.
                let _ = (mb, profile);
                "Detected RAM — heap and GC tuned automatically"
            }
            Self::Performance => "Max FPS, 4GB RAM, G1GC tuned",
            Self::Balanced => "Good FPS, 2GB RAM, standard G1GC",
            Self::Quality => "Best visuals, 6GB RAM, large heap",
            Self::Potato => "Minimum RAM, serial GC for low-end PCs",
        }
    }
}

pub mod chunk {
    /// Returns the optimal render distance for a given profile.
    pub fn recommended_render_distance(profile: &super::OptimizationProfile) -> u8 {
        match profile {
            super::OptimizationProfile::Auto        => recommended_render_distance(&super::auto_tune()),
            super::OptimizationProfile::Performance => 12,
            super::OptimizationProfile::Balanced    => 10,
            super::OptimizationProfile::Quality     => 16,
            super::OptimizationProfile::Potato      => 6,
        }
    }
}

pub mod memory {
    /// Returns max heap in MB for a given profile.
    pub fn max_heap_mb(profile: &super::OptimizationProfile) -> u32 {
        match profile {
            super::OptimizationProfile::Auto        => super::auto_heap_mb() as u32,
            super::OptimizationProfile::Performance => 4096,
            super::OptimizationProfile::Balanced    => 2048,
            super::OptimizationProfile::Quality     => 6144,
            super::OptimizationProfile::Potato      => 512,
        }
    }
}

pub mod rendering {
    /// Returns whether entity culling should be enabled.
    pub fn entity_culling(profile: &super::OptimizationProfile) -> bool {
        !matches!(profile, super::OptimizationProfile::Quality)
    }

    /// Returns whether fast math optimizations should be applied.
    pub fn fast_math(profile: &super::OptimizationProfile) -> bool {
        matches!(
            profile,
            super::OptimizationProfile::Performance | super::OptimizationProfile::Potato
        )
    }
}
