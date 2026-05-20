use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    Project,
    Global,
}

pub fn global_memory_path(data_dir: &str) -> PathBuf {
    PathBuf::from(data_dir).join("memory").join("global.md")
}

pub fn project_memory_path() -> PathBuf {
    let root = find_project_root().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    root.join(".zero-agent").join("memory").join("project.md")
}

pub fn find_project_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    let home = std::env::var("HOME").ok().map(PathBuf::from);

    loop {
        if dir.join(".git").exists()
            || dir.join("go.mod").exists()
            || dir.join(".zero-agent").exists()
        {
            return Some(dir);
        }
        if home.as_ref().is_some_and(|h| dir == *h) || dir.parent().is_none() {
            return Some(std::env::current_dir().ok()?);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

pub fn append_memory(scope: MemoryScope, text: &str, data_dir: &str) -> Result<(String, String), String> {
    let path = match scope {
        MemoryScope::Project => project_memory_path(),
        MemoryScope::Global => global_memory_path(data_dir),
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create memory dir: {e}"))?;
    }

    let other_path = match scope {
        MemoryScope::Project => global_memory_path(data_dir),
        MemoryScope::Global => project_memory_path(),
    };

    if other_path.exists() {
        let existing = std::fs::read_to_string(&other_path).unwrap_or_default();
        if conflicts(&existing, text) {
            return Err(format!(
                "Memory conflict: new fact may contradict existing entry in {}. Clarify before saving.",
                other_path.display()
            ));
        }
    }

    let entry = format!("\n- {}\n", text.trim());
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("failed to open memory file: {e}"))?;
    file.write_all(entry.as_bytes())
        .map_err(|e| format!("failed to write memory: {e}"))?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("memory.md")
        .to_string();
    let description = truncate_description(text, 60);
    Ok((description, file_name))
}

pub fn list_memories(data_dir: &str) -> String {
    let mut out = String::new();
    let project = project_memory_path();
    let global = global_memory_path(data_dir);

    if project.exists() {
        out.push_str(&format!("Project ({}):\n", project.display()));
        out.push_str(&std::fs::read_to_string(&project).unwrap_or_default());
        out.push('\n');
    }
    if global.exists() {
        out.push_str(&format!("Global ({}):\n", global.display()));
        out.push_str(&std::fs::read_to_string(&global).unwrap_or_default());
    }
    if out.is_empty() {
        out.push_str("No memories saved yet.");
    }
    out
}

fn conflicts(existing: &str, new_text: &str) -> bool {
    let new_lower = new_text.to_lowercase();
    for line in existing.lines() {
        let line = line.trim().trim_start_matches('-').trim().to_lowercase();
        if line.is_empty() {
            continue;
        }
        if (line.contains("prefer") && new_lower.contains("prefer"))
            || (line.contains("always") && new_lower.contains("never"))
            || (line.contains("never") && new_lower.contains("always"))
        {
            return true;
        }
    }
    false
}

fn truncate_description(text: &str, max: usize) -> String {
    let one_line = text.lines().next().unwrap_or(text).trim();
    if one_line.len() <= max {
        one_line.to_string()
    } else {
        format!("{}...", &one_line[..max.saturating_sub(3)])
    }
}
