// src/library.rs — scans the soundfont library directory

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A single soundfont folder discovered in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundFont {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub file_count: u64,
}

impl SoundFont {
    pub fn size_human(&self) -> String {
        human_bytes(self.size_bytes)
    }
}

/// Walk `library_dir` recursively and return a SoundFont for every folder
/// that contains a `config.ini` file directly inside it (not deeper).
/// The font's `name` is the relative path from the library root with
/// path separators replaced by `_`, e.g. `Tales_Bundle_Dooku`.
pub fn scan_library(library_dir: &Path) -> Result<Vec<SoundFont>> {
    let mut fonts: Vec<SoundFont> = Vec::new();

    for entry in WalkDir::new(library_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.file_name() == "config.ini")
    {
        // The soundfont folder is the parent of config.ini
        let folder_path = match entry.path().parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };

        // Build a label: relative path from library root, slashes → underscores
        let rel = match folder_path.strip_prefix(library_dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("_");

        if name.is_empty() {
            continue; // config.ini sitting directly in the library root — skip
        }

        let (size_bytes, file_count) = dir_stats(&folder_path);
        fonts.push(SoundFont { name, path: folder_path, size_bytes, file_count });
    }

    // Sort alphabetically by label
    fonts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(fonts)
}

/// Export fonts from a profile into `dest_dir/profile_name/`, naming each
/// destination folder with its 1-based slot number: `1-LEAFNAME`, `2-LEAFNAME`, …
///
/// The slot order comes directly from the `font_names` slice (index 0 = slot 1).
/// The overall library directory structure is **not** renumbered or altered.
pub fn export_profile_numbered(
    library: &[SoundFont],
    dest_dir: &Path,
    profile_name: &str,
    font_names: &[String],
) -> Result<Vec<String>> {
    let out = dest_dir.join(profile_name);
    std::fs::create_dir_all(&out)
        .with_context(|| format!("Cannot create output dir {}", out.display()))?;

    let mut log = Vec::new();
    for (slot, name) in font_names.iter().enumerate() {
        match library.iter().find(|f| &f.name == name) {
            None => {
                log.push(format!("SKIP {name} (not found in library)"));
            }
            Some(font) => {
                // Leaf folder name from the actual path
                let leaf = font
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| name.clone());

                // Destination: "<slot+1>-<leaf>"
                let dst_name = format!("{}-{}", slot + 1, leaf);
                let dst = out.join(&dst_name);

                copy_dir_all(&font.path, &dst)
                    .with_context(|| format!("Failed to copy {name}"))?;

                log.push(format!("✓ {name}  →  {dst_name}"));
            }
        }
    }
    Ok(log)
}

/// Recursively copy a directory tree from `src` to `dst`.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

// ─── Shared utilities ─────────────────────────────────────────────────────────

/// Recursively compute (total_bytes, file_count) for a directory.
fn dir_stats(dir: &Path) -> (u64, u64) {
    let mut total = 0u64;
    let mut count = 0u64;
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            count += 1;
        }
    }
    (total, count)
}

/// Convert bytes to a human-readable string.
pub fn human_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB      { format!("{:.2} GB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.2} MB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.1} KB", bytes as f64 / KB as f64) }
    else                { format!("{} B", bytes) }
}
