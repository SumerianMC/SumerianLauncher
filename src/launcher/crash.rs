use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct CrashReport {
    pub path: PathBuf,
    pub description: String,
    pub exception: Option<String>,
    pub suspected_mods: Vec<String>,
    pub diagnoses: Vec<Diagnosis>,
}

#[derive(Debug)]
pub struct Diagnosis {
    pub cause: &'static str,
    pub fix: &'static str,
}

/// Find the newest crash report in `game_dir/crash-reports/`.
pub fn find_latest(game_dir: &Path) -> Option<PathBuf> {
    let dir = game_dir.join("crash-reports");
    std::fs::read_dir(&dir).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("txt"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// Parse a crash report file into a structured `CrashReport`.
pub fn parse(path: &Path) -> Option<CrashReport> {
    let content = std::fs::read_to_string(path).ok()?;

    let description = content
        .lines()
        .find(|l| l.starts_with("Description:"))
        .map(|l| l.trim_start_matches("Description:").trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // First exception line (java.lang.* or similar)
    let exception = content
        .lines()
        .find(|l| {
            let t = l.trim();
            t.starts_with("java.") || t.starts_with("net.minecraft.") || t.starts_with("Caused by:")
        })
        .map(|l| l.trim().to_string());

    // Mods section: lines between "-- Mods --" and next "--" header
    let suspected_mods = parse_suspected_mods(&content);

    let diagnoses = diagnose(&content);

    Some(CrashReport {
        path: path.to_path_buf(),
        description,
        exception,
        suspected_mods,
        diagnoses,
    })
}

fn parse_suspected_mods(content: &str) -> Vec<String> {
    let mut in_mods = false;
    let mut mods = Vec::new();
    for line in content.lines() {
        if line.trim() == "-- Mods --" || line.contains("Loaded mods:") {
            in_mods = true;
            continue;
        }
        if in_mods {
            if line.starts_with("--") { break; }
            let t = line.trim();
            if !t.is_empty() && !t.starts_with("//") {
                mods.push(t.to_string());
            }
        }
    }
    // Also scan stack trace for mod jar names (e.g. "at com.example.mod")
    if mods.is_empty() {
        for line in content.lines() {
            let t = line.trim();
            if t.starts_with("at ") && !t.contains("net.minecraft") && !t.contains("java.") && !t.contains("sun.") {
                let class = t.trim_start_matches("at ").split('(').next().unwrap_or("").to_string();
                if !class.is_empty() && !mods.contains(&class) {
                    mods.push(class);
                    if mods.len() >= 5 { break; }
                }
            }
        }
    }
    mods
}

fn diagnose(content: &str) -> Vec<Diagnosis> {
    let mut out = Vec::new();
    let c = content.to_lowercase();

    if c.contains("outofmemoryerror") {
        out.push(Diagnosis {
            cause: "Out of memory",
            fix: "Increase heap size — switch to Performance or Quality profile, or set a higher -Xmx value in instance JVM args.",
        });
    }
    if c.contains("stackoverflowerror") {
        out.push(Diagnosis {
            cause: "Stack overflow",
            fix: "Usually caused by a mod with infinite recursion. Try disabling mods one by one.",
        });
    }
    if c.contains("classnotfoundexception") || c.contains("noclassdeffounderror") {
        out.push(Diagnosis {
            cause: "Missing class — mod or library not found",
            fix: "A mod dependency is missing. Check that all required mods and their correct versions are installed.",
        });
    }
    if c.contains("unsupportedclassversionerror") {
        out.push(Diagnosis {
            cause: "Wrong Java version",
            fix: "The mod or game requires a newer Java. Check the Java version requirement and update via Manage Instances → Edit profile → Java path.",
        });
    }
    if c.contains("opengl") || c.contains("lwjgl") {
        out.push(Diagnosis {
            cause: "OpenGL / LWJGL error",
            fix: "Update your GPU drivers. If using shaders, try disabling them via Manage Shaders.",
        });
    }
    if c.contains("mixinerror") || c.contains("mixin apply") {
        out.push(Diagnosis {
            cause: "Mixin conflict between mods",
            fix: "Two mods are incompatible. Check mod changelogs for known conflicts and remove one of the conflicting mods.",
        });
    }
    if c.contains("nullpointerexception") {
        out.push(Diagnosis {
            cause: "Null pointer exception",
            fix: "Often a mod bug or version mismatch. Check that all mods are compatible with your Minecraft version.",
        });
    }
    if c.contains("failed to create display") || c.contains("could not create context") {
        out.push(Diagnosis {
            cause: "Display / graphics context creation failed",
            fix: "Update GPU drivers. On Windows, try running with -Djava.awt.headless=false in JVM args.",
        });
    }
    if c.contains("connection refused") || c.contains("unknownhostexception") {
        out.push(Diagnosis {
            cause: "Network error",
            fix: "Check your internet connection. If joining a server, verify the address and port are correct.",
        });
    }
    if out.is_empty() {
        out.push(Diagnosis {
            cause: "Unknown crash",
            fix: "Check the full crash report for the exception and stack trace. Search the exception message online for more help.",
        });
    }
    out
}
