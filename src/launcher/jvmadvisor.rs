/// JVM flag advisor.
///
/// Inspects the detected Java version, available system RAM, and chosen
/// OptimizationProfile to produce a tailored set of JVM flags with explanations.
/// Returns both a copyable flag string and a human-readable rationale list.
use crate::optimizer::{OptimizationProfile, auto_heap_mb, system_ram_mb};

#[derive(Debug, Clone)]
pub struct FlagAdvice {
    pub flag: String,
    pub reason: String,
}

pub struct JvmAdvice {
    pub flags: Vec<FlagAdvice>,
    /// Ready-to-paste single-line flag string.
    pub command_line: String,
}

/// Generate advised JVM flags for the given Java major version and profile.
pub fn advise(java_major: u32, profile: &OptimizationProfile) -> JvmAdvice {
    let ram_mb = system_ram_mb();
    let heap_mb = auto_heap_mb();
    let mut advice: Vec<FlagAdvice> = Vec::new();

    // ── Heap sizing ──────────────────────────────────────────────────────────
    let xmx = match profile {
        OptimizationProfile::Potato      => 512u64,
        OptimizationProfile::Balanced    => 2048,
        OptimizationProfile::Performance => 4096,
        OptimizationProfile::Quality     => 6144,
        OptimizationProfile::Auto        => heap_mb,
    };
    let xms = (xmx / 8).max(256);
    advice.push(FlagAdvice {
        flag: format!("-Xmx{}M -Xms{}M", xmx, xms),
        reason: format!("Max heap {}MB, initial {}MB — system has {}MB total", xmx, xms, ram_mb),
    });

    // ── GC selection ─────────────────────────────────────────────────────────
    if java_major >= 21 && xmx >= 2048 {
        advice.push(FlagAdvice {
            flag: "-XX:+UseZGC -XX:+ZGenerational".into(),
            reason: "Java 21+ ZGC with generational mode — ultra-low pause times, ideal for smooth gameplay".into(),
        });
    } else if java_major >= 15 && xmx >= 2048 {
        advice.push(FlagAdvice {
            flag: "-XX:+UseZGC".into(),
            reason: "ZGC available on Java 15+ — very low GC pauses".into(),
        });
    } else if java_major >= 11 {
        advice.push(FlagAdvice {
            flag: "-XX:+UseG1GC -XX:G1HeapRegionSize=16M -XX:MaxGCPauseMillis=50 -XX:+UnlockExperimentalVMOptions".into(),
            reason: "G1GC with 50ms pause target — best balance on Java 11+".into(),
        });
    } else {
        // Java 8
        advice.push(FlagAdvice {
            flag: "-XX:+UseG1GC -XX:MaxGCPauseMillis=100 -XX:+UnlockExperimentalVMOptions".into(),
            reason: "G1GC on Java 8 — solid choice for Minecraft 1.0–1.16".into(),
        });
    }

    // ── String deduplication (Java 8u20+, G1GC only) ─────────────────────────
    if java_major >= 8 && java_major < 15 {
        advice.push(FlagAdvice {
            flag: "-XX:+UseStringDeduplication".into(),
            reason: "Deduplicate identical String objects — saves 5–15% heap on mod-heavy installs".into(),
        });
    }

    // ── Codegen & startup ─────────────────────────────────────────────────────
    advice.push(FlagAdvice {
        flag: "-XX:+OptimizeStringConcat -XX:+UseCompressedOops".into(),
        reason: "Faster string concatenation and compressed object pointers".into(),
    });

    if java_major >= 11 {
        advice.push(FlagAdvice {
            flag: "-XX:+AlwaysPreTouch".into(),
            reason: "Pre-touch heap pages at startup — reduces GC stutter mid-session".into(),
        });
        advice.push(FlagAdvice {
            flag: "-XX:+ParallelRefProcEnabled".into(),
            reason: "Process soft/weak references in parallel — shorter GC pauses".into(),
        });
    }

    // ── Module/JVM warnings suppression (Java 9+) ────────────────────────────
    if java_major >= 9 {
        advice.push(FlagAdvice {
            flag: "--add-opens java.base/sun.security.util=ALL-UNNAMED \
                   --add-opens java.base/java.util=ALL-UNNAMED"
                .into(),
            reason: "Suppress illegal-access warnings from Minecraft and mods on Java 9+".into(),
        });
    }

    // ── Potato-mode extras ────────────────────────────────────────────────────
    if matches!(profile, OptimizationProfile::Potato) {
        advice.push(FlagAdvice {
            flag: "-XX:+UseSerialGC -client".into(),
            reason: "Serial GC and client JIT — lowest overhead for very low-end hardware".into(),
        });
    }

    // Build copyable command line
    let command_line = advice.iter().map(|a| a.flag.as_str()).collect::<Vec<_>>().join(" ");

    JvmAdvice { flags: advice, command_line }
}
