// src/app.rs — TUI state machine

use std::collections::HashSet;
use std::path::PathBuf;

use crate::library::{export_profile_numbered, human_bytes, SoundFont};
use crate::profile::{diff_profiles, Profile, ProfileDiff};

/// Which screen the TUI is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Browser,
    SaveDialog,
    LoadDialog,
    /// Full-screen profile editor: shows the ordered font list for the current
    /// working selection and lets the user reorder entries before saving.
    ProfileEditor,
    DiffPick,
    DiffView,
    /// Pick a saved profile to export to a folder
    ExportPick,
    /// Scrollable result log (export operations)
    ResultLog,
    Popup,
}

pub struct App {
    // ── library ──────────────────────────────────────────────────────────────
    pub library: Vec<SoundFont>,
    pub library_path: PathBuf,

    // ── browser state ────────────────────────────────────────────────────────
    pub cursor: usize,
    pub selected: HashSet<String>,

    // ── profile management ───────────────────────────────────────────────────
    /// Display name of the current working profile.
    pub profile_name: String,
    /// Ordered list for the current working profile (drives export slot numbers).
    pub profile_order: Vec<String>,
    pub profiles_dir: PathBuf,
    pub saved_profiles: Vec<PathBuf>,

    // ── screens ──────────────────────────────────────────────────────────────
    pub screen: Screen,

    // ── text input (save dialog) ──────────────────────────────────────────────
    pub input_buffer: String,

    // ── load / export / diff-pick dialog ─────────────────────────────────────
    pub load_cursor: usize,

    // ── profile editor ────────────────────────────────────────────────────────
    /// Cursor position inside the profile editor list.
    pub editor_cursor: usize,
    /// When Some(i), the item at index i is being dragged (move mode).
    pub editor_drag: Option<usize>,

    // ── diff ─────────────────────────────────────────────────────────────────
    pub diff_pick_step: u8,
    pub diff_profile_a: Option<PathBuf>,
    pub diff_result: Option<DiffDisplay>,

    // ── result log ───────────────────────────────────────────────────────────
    pub result_log: Vec<String>,
    pub log_scroll: usize,

    // ── popup ────────────────────────────────────────────────────────────────
    pub popup_message: String,
}

pub struct DiffDisplay {
    pub name_a: String,
    pub name_b: String,
    pub diff: ProfileDiff,
    pub size_a: u64,
    pub size_b: u64,
    pub files_a: u64,
    pub files_b: u64,
}

impl App {
    pub fn new(library: Vec<SoundFont>, library_path: PathBuf, profiles_dir: PathBuf) -> Self {
        let saved_profiles = Self::scan_profiles(&profiles_dir);
        App {
            library,
            library_path,
            cursor: 0,
            selected: HashSet::new(),
            profile_name: "new_profile".into(),
            profile_order: Vec::new(),
            profiles_dir,
            saved_profiles,
            screen: Screen::Browser,
            input_buffer: String::new(),
            load_cursor: 0,
            editor_cursor: 0,
            editor_drag: None,
            diff_pick_step: 0,
            diff_profile_a: None,
            diff_result: None,
            result_log: Vec::new(),
            log_scroll: 0,
            popup_message: String::new(),
        }
    }

    // ── profile helpers ───────────────────────────────────────────────────────

    pub fn scan_profiles(dir: &PathBuf) -> Vec<PathBuf> {
        match std::fs::read_dir(dir) {
            Ok(rd) => {
                let mut paths: Vec<PathBuf> = rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
                    .collect();
                paths.sort();
                paths
            }
            Err(_) => vec![],
        }
    }

    pub fn reload_profiles(&mut self) {
        self.saved_profiles = Self::scan_profiles(&self.profiles_dir);
    }

    pub fn reload_library(&mut self) {
        if let Ok(lib) = crate::library::scan_library(&self.library_path) {
            self.library = lib;
            if self.cursor >= self.library.len() {
                self.cursor = self.library.len().saturating_sub(1);
            }
        }
    }

    /// Build a Profile from the current selection using `profile_order` for
    /// ordering.  Any selected font not yet in `profile_order` is appended
    /// alphabetically so nothing is silently dropped.
    pub fn working_profile(&self) -> Profile {
        // Start with the saved order, keeping only still-selected fonts
        let mut fonts: Vec<String> = self
            .profile_order
            .iter()
            .filter(|n| self.selected.contains(*n))
            .cloned()
            .collect();

        // Append any newly-selected fonts not already in the order list
        let in_order: HashSet<&String> = fonts.iter().collect();
        let mut extras: Vec<String> = self
            .selected
            .iter()
            .filter(|n| !in_order.contains(*n))
            .cloned()
            .collect();
        extras.sort();
        fonts.extend(extras);

        Profile {
            name: self.profile_name.clone(),
            description: None,
            fonts,
        }
    }

    /// Open the profile editor for the current working selection.
    /// Syncs `profile_order` to match the working profile and opens the editor.
    pub fn open_profile_editor(&mut self) {
        let profile = self.working_profile();
        self.profile_order = profile.fonts;
        self.editor_cursor = 0;
        self.editor_drag = None;
        self.screen = Screen::ProfileEditor;
    }

    pub fn save_profile(&mut self) -> Result<(), String> {
        let name = self.input_buffer.trim().to_string();
        if name.is_empty() {
            return Err("Profile name cannot be empty".into());
        }
        std::fs::create_dir_all(&self.profiles_dir)
            .map_err(|e| format!("Cannot create profiles dir: {e}"))?;

        // Build profile using the current editor order
        let fonts = self.profile_order
            .iter()
            .filter(|n| self.selected.contains(*n))
            .cloned()
            .collect::<Vec<_>>();

        let profile = Profile {
            name: name.clone(),
            description: None,
            fonts,
        };
        self.profile_name = name.clone();
        let path = self.profiles_dir.join(format!("{name}.toml"));
        profile.save(&path).map_err(|e| format!("{e}"))?;
        self.reload_profiles();
        Ok(())
    }

    pub fn load_selected_profile(&mut self) -> Result<(), String> {
        let path = self
            .saved_profiles
            .get(self.load_cursor)
            .cloned()
            .ok_or("No profile selected")?;
        let profile = Profile::load(&path).map_err(|e| format!("{e}"))?;
        self.profile_name = profile.name.clone();
        self.selected = profile.fonts.iter().cloned().collect();
        // Preserve the saved order exactly
        self.profile_order = profile.fonts;
        Ok(())
    }

    // ── profile editor actions ────────────────────────────────────────────────

    pub fn editor_cursor_up(&mut self) {
        if self.editor_cursor > 0 {
            self.editor_cursor -= 1;
            // If dragging, move the item too
            if let Some(drag) = self.editor_drag {
                self.profile_order.swap(drag, drag - 1);
                self.editor_drag = Some(drag - 1);
            }
        }
    }

    pub fn editor_cursor_down(&mut self) {
        let max = self.profile_order.len().saturating_sub(1);
        if self.editor_cursor < max {
            self.editor_cursor += 1;
            if let Some(drag) = self.editor_drag {
                self.profile_order.swap(drag, drag + 1);
                self.editor_drag = Some(drag + 1);
            }
        }
    }

    /// Toggle grab/release of the item under the cursor.
    pub fn editor_toggle_drag(&mut self) {
        if self.editor_drag.is_some() {
            self.editor_drag = None;
        } else if !self.profile_order.is_empty() {
            self.editor_drag = Some(self.editor_cursor);
        }
    }

    /// Remove the font under the cursor from the profile_order (and selection).
    pub fn editor_remove_item(&mut self) {
        if self.profile_order.is_empty() {
            return;
        }
        let name = self.profile_order.remove(self.editor_cursor);
        self.selected.remove(&name);
        if self.editor_cursor > 0 && self.editor_cursor >= self.profile_order.len() {
            self.editor_cursor -= 1;
        }
        self.editor_drag = None;
    }

    // ── cursor helpers ────────────────────────────────────────────────────────

    pub fn toggle_selected(&mut self) {
        if let Some(font) = self.library.get(self.cursor) {
            let name = font.name.clone();
            if self.selected.contains(&name) {
                self.selected.remove(&name);
                self.profile_order.retain(|n| n != &name);
            } else {
                self.selected.insert(name.clone());
                // Append to end of order if not already there
                if !self.profile_order.contains(&name) {
                    self.profile_order.push(name);
                }
            }
        }
    }

    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }
    pub fn cursor_down(&mut self) {
        if self.cursor + 1 < self.library.len() {
            self.cursor += 1;
        }
    }
    pub fn load_cursor_up(&mut self) {
        if self.load_cursor > 0 {
            self.load_cursor -= 1;
        }
    }
    pub fn load_cursor_down(&mut self) {
        if self.load_cursor + 1 < self.saved_profiles.len() {
            self.load_cursor += 1;
        }
    }

    // ── summary helpers ───────────────────────────────────────────────────────

    pub fn total_selected_size(&self) -> u64 {
        self.library
            .iter()
            .filter(|f| self.selected.contains(&f.name))
            .map(|f| f.size_bytes)
            .sum()
    }
    pub fn total_selected_files(&self) -> u64 {
        self.library
            .iter()
            .filter(|f| self.selected.contains(&f.name))
            .map(|f| f.file_count)
            .sum()
    }
    pub fn selected_size_human(&self) -> String {
        human_bytes(self.total_selected_size())
    }

    // ── diff ─────────────────────────────────────────────────────────────────

    pub fn diff_select(&mut self) {
        if self.diff_pick_step == 0 {
            if let Some(path) = self.saved_profiles.get(self.load_cursor).cloned() {
                self.diff_profile_a = Some(path);
                self.diff_pick_step = 1;
            }
        } else {
            let path_b = match self.saved_profiles.get(self.load_cursor).cloned() {
                Some(p) => p,
                None => return,
            };
            let path_a = match &self.diff_profile_a {
                Some(p) => p.clone(),
                None => return,
            };
            let prof_a = match Profile::load(&path_a) {
                Ok(p) => p,
                Err(e) => {
                    self.popup_message = format!("Error loading A: {e}");
                    self.screen = Screen::Popup;
                    return;
                }
            };
            let prof_b = match Profile::load(&path_b) {
                Ok(p) => p,
                Err(e) => {
                    self.popup_message = format!("Error loading B: {e}");
                    self.screen = Screen::Popup;
                    return;
                }
            };
            let diff = diff_profiles(&prof_a, &prof_b);
            self.diff_result = Some(DiffDisplay {
                size_a: prof_a.total_size(&self.library),
                size_b: prof_b.total_size(&self.library),
                files_a: prof_a.total_files(&self.library),
                files_b: prof_b.total_files(&self.library),
                name_a: prof_a.name.clone(),
                name_b: prof_b.name.clone(),
                diff,
            });
            self.diff_pick_step = 0;
            self.diff_profile_a = None;
            self.screen = Screen::DiffView;
        }
    }

    // ── export ────────────────────────────────────────────────────────────────

    /// Export the profile at `saved_profiles[load_cursor]`.
    /// Folder names in the export are prefixed with their slot number: "1-NAME".
    pub fn confirm_export(&mut self) {
        let path = match self.saved_profiles.get(self.load_cursor).cloned() {
            Some(p) => p,
            None => {
                self.popup_message = "No profile selected.".into();
                self.screen = Screen::Popup;
                return;
            }
        };
        let profile = match Profile::load(&path) {
            Ok(p) => p,
            Err(e) => {
                self.popup_message = format!("Load failed: {e}");
                self.screen = Screen::Popup;
                return;
            }
        };
        let dest = self
            .library_path
            .parent()
            .unwrap_or(&self.library_path)
            .join("profiles");
        match export_profile_numbered(&self.library, &dest, &profile.name, &profile.fonts) {
            Ok(log) => {
                let out = dest.join(&profile.name);
                self.result_log = log;
                self.result_log.insert(
                    0,
                    format!("Exported \"{}\" → {}", profile.name, out.display()),
                );
                self.log_scroll = 0;
                self.screen = Screen::ResultLog;
            }
            Err(e) => {
                self.popup_message = format!("Export failed: {e}");
                self.screen = Screen::Popup;
            }
        }
    }

    pub fn log_scroll_up(&mut self) {
        if self.log_scroll > 0 {
            self.log_scroll -= 1;
        }
    }
    pub fn log_scroll_down(&mut self, max: usize) {
        if self.log_scroll + 1 < max {
            self.log_scroll += 1;
        }
    }
}
