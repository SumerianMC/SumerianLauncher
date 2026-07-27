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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OptimizationProfile {
    Performance,
    Balanced,
    Quality,
    Potato,
}

impl fmt::Display for OptimizationProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Performance,
            Self::Balanced,
            Self::Quality,
            Self::Potato,
        ]
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Performance,
            1 => Self::Balanced,
            2 => Self::Quality,
            _ => Self::Potato,
        }
    }

    /// Returns JVM flags for this profile.
    pub fn jvm_flags(&self) -> Vec<String> {
        match self {
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
            super::OptimizationProfile::Performance => 12,
            super::OptimizationProfile::Balanced => 10,
            super::OptimizationProfile::Quality => 16,
            super::OptimizationProfile::Potato => 6,
        }
    }
}

pub mod memory {
    /// Returns max heap in MB for a given profile.
    pub fn max_heap_mb(profile: &super::OptimizationProfile) -> u32 {
        match profile {
            super::OptimizationProfile::Performance => 4096,
            super::OptimizationProfile::Balanced => 2048,
            super::OptimizationProfile::Quality => 6144,
            super::OptimizationProfile::Potato => 512,
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
