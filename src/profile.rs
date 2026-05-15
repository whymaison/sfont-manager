// src/profile.rs — Profile save / load / diff

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::library::SoundFont;

/// A saved profile: an ordered list of soundfont names + optional description.
/// The order of `fonts` is the canonical slot order (slot 1 = index 0, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: Option<String>,
    /// Ordered list of SoundFont folder names in this profile.
    /// Position in the Vec = slot number - 1.
    pub fonts: Vec<String>,
}

impl Profile {
    pub fn new(name: impl Into<String>) -> Self {
        Profile {
            name: name.into(),
            description: None,
            fonts: Vec::new(),
        }
    }

    /// Save profile to a TOML file at `path`.
    pub fn save(&self, path: &Path) -> Result<()> {
        let toml = toml::to_string_pretty(self)
            .context("Failed to serialize profile")?;
        std::fs::write(path, toml)
            .with_context(|| format!("Failed to write profile to {}", path.display()))?;
        Ok(())
    }

    /// Load a profile from a TOML file at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read profile from {}", path.display()))?;
        let profile: Profile = toml::from_str(&data)
            .context("Failed to parse profile TOML")?;
        Ok(profile)
    }

    /// Total size of all fonts in this profile, given the full library list.
    pub fn total_size(&self, library: &[SoundFont]) -> u64 {
        self.fonts.iter()
            .filter_map(|name| library.iter().find(|f| &f.name == name))
            .map(|f| f.size_bytes)
            .sum()
    }

    /// Total file count across all fonts in this profile.
    pub fn total_files(&self, library: &[SoundFont]) -> u64 {
        self.fonts.iter()
            .filter_map(|name| library.iter().find(|f| &f.name == name))
            .map(|f| f.file_count)
            .sum()
    }

    /// Quickly load just font count for the load dialog (no library needed).
    pub fn load_meta(path: &Path) -> Option<usize> {
        Self::load(path).ok().map(|p| p.fonts.len())
    }
}

/// The result of comparing two profiles.
pub struct ProfileDiff {
    pub only_in_a: Vec<String>,
    pub only_in_b: Vec<String>,
    pub in_both: Vec<String>,
}

/// Compute which fonts are unique to each profile and which are shared.
pub fn diff_profiles(a: &Profile, b: &Profile) -> ProfileDiff {
    let set_a: HashSet<&String> = a.fonts.iter().collect();
    let set_b: HashSet<&String> = b.fonts.iter().collect();

    let mut only_in_a: Vec<String> = set_a.difference(&set_b).map(|s| (*s).clone()).collect();
    let mut only_in_b: Vec<String> = set_b.difference(&set_a).map(|s| (*s).clone()).collect();
    let mut in_both: Vec<String>   = set_a.intersection(&set_b).map(|s| (*s).clone()).collect();

    only_in_a.sort();
    only_in_b.sort();
    in_both.sort();

    ProfileDiff { only_in_a, only_in_b, in_both }
}
