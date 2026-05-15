// src/main.rs

mod app;
mod library;
mod profile;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, Screen};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Get user home dir
    let home_dir = dirs::home_dir()
        .context("Could not determine the user's home directory")?;

    // Default library path: ~/Documents/Lightsabers/SoundFonts/
    let default_library_path = home_dir
        .join("Documents")
        .join("Lightsabers")
        .join("SoundFonts");
    
    // Use CLI arg if provided, otherwise fall back to default
    let library_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or(default_library_path);

    // Profiles directory should live beside the library directory:
    //
    // Example:
    // library  -> /temp/sounds
    // profiles -> /temp/profiles
    //
    // Default case:
    // library  -> ~/Documents/Lightsabers/SoundFonts
    // profiles -> ~/Documents/Lightsabers/profiles
    let profiles_dir = library_path
        .parent()
        .context("Library path has no parent directory")?
        .join("profiles");

    if !library_path.is_dir() {
        anyhow::bail!("'{}' is not a directory.", library_path.display());
    }

    eprintln!("Scanning library at {} ...", library_path.display());
    let library = library::scan_library(&library_path)
        .with_context(|| format!("Failed to scan '{}'", library_path.display()))?;

    if library.is_empty() {
        anyhow::bail!(
            "No soundfont folders found in '{}'.",
            library_path.display()
        );
    }

    eprintln!("Found {} soundfont folders. Starting TUI...", library.len());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(library, library_path, profiles_dir);
    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        eprintln!("Error: {e}");
    }
    result
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match app.screen.clone() {
            // ── Browser ───────────────────────────────────────────────────────
            Screen::Browser => match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => break,
                KeyCode::Char('c') if ctrl => break,

                KeyCode::Up | KeyCode::Char('k') => app.cursor_up(),
                KeyCode::Down | KeyCode::Char('j') => app.cursor_down(),
                KeyCode::Char(' ') => app.toggle_selected(),

                KeyCode::Char('a') | KeyCode::Char('A') => {
                    for font in app.library.iter() {
                        let name = font.name.clone();
                        if !app.selected.contains(&name) {
                            app.selected.insert(name.clone());
                            if !app.profile_order.contains(&name) {
                                app.profile_order.push(name);
                            }
                        }
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    app.selected.clear();
                    app.profile_order.clear();
                }

                // Open profile editor for the current working selection
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    if app.selected.is_empty() {
                        app.popup_message = "Nothing selected — toggle some fonts first.".into();
                        app.screen = Screen::Popup;
                    } else {
                        app.open_profile_editor();
                    }
                }

                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if app.selected.is_empty() {
                        app.popup_message = "Nothing selected to save.".into();
                        app.screen = Screen::Popup;
                    } else {
                        // Sync profile_order before entering save dialog
                        let profile = app.working_profile();
                        app.profile_order = profile.fonts;
                        app.input_buffer = app.profile_name.clone();
                        app.screen = Screen::SaveDialog;
                    }
                }
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    app.reload_profiles();
                    app.load_cursor = 0;
                    app.screen = Screen::LoadDialog;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    app.reload_profiles();
                    if app.saved_profiles.len() < 2 {
                        app.popup_message = "Need at least 2 saved profiles to diff.".into();
                        app.screen = Screen::Popup;
                    } else {
                        app.load_cursor = 0;
                        app.diff_pick_step = 0;
                        app.diff_profile_a = None;
                        app.screen = Screen::DiffPick;
                    }
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    app.reload_profiles();
                    if app.saved_profiles.is_empty() {
                        app.popup_message = "No saved profiles to export.".into();
                        app.screen = Screen::Popup;
                    } else {
                        app.load_cursor = 0;
                        app.screen = Screen::ExportPick;
                    }
                }
                _ => {}
            },

            // ── Save dialog ───────────────────────────────────────────────────
            Screen::SaveDialog => match key.code {
                KeyCode::Esc => app.screen = Screen::Browser,
                KeyCode::Enter => {
                    match app.save_profile() {
                        Ok(()) => {
                            app.popup_message = format!(
                                "Saved \"{}\"  ·  {} fonts  ·  {}",
                                app.profile_name,
                                app.profile_order.len(),
                                app.selected_size_human(),
                            )
                        }
                        Err(e) => app.popup_message = format!("Save failed: {e}"),
                    }
                    app.screen = Screen::Popup;
                }
                KeyCode::Backspace => {
                    app.input_buffer.pop();
                }
                KeyCode::Char(c) => app.input_buffer.push(c),
                _ => {}
            },

            // ── Load dialog ───────────────────────────────────────────────────
            // After loading, jump straight into the profile editor so the user
            // can inspect/reorder before anything is changed in the browser.
            Screen::LoadDialog => match key.code {
                KeyCode::Esc => app.screen = Screen::Browser,
                KeyCode::Up | KeyCode::Char('k') => app.load_cursor_up(),
                KeyCode::Down | KeyCode::Char('j') => app.load_cursor_down(),
                KeyCode::Enter => {
                    match app.load_selected_profile() {
                        Ok(()) => {
                            // Drop into the editor so the user can reorder / confirm
                            app.editor_cursor = 0;
                            app.editor_drag = None;
                            app.screen = Screen::ProfileEditor;
                        }
                        Err(e) => {
                            app.popup_message = format!("Load failed: {e}");
                            app.screen = Screen::Popup;
                        }
                    }
                }
                _ => {}
            },

            // ── Profile editor ────────────────────────────────────────────────
            Screen::ProfileEditor => match key.code {
                KeyCode::Esc => {
                    // Drop drag state, return to browser; selection/order kept
                    app.editor_drag = None;
                    app.screen = Screen::Browser;
                }

                KeyCode::Up | KeyCode::Char('k') => app.editor_cursor_up(),
                KeyCode::Down | KeyCode::Char('j') => app.editor_cursor_down(),

                // Grab / drop
                KeyCode::Char(' ') => app.editor_toggle_drag(),

                // Remove item from profile
                KeyCode::Delete | KeyCode::Char('x') | KeyCode::Char('X') => {
                    app.editor_remove_item();
                }

                // Save directly from the editor
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    app.input_buffer = app.profile_name.clone();
                    app.screen = Screen::SaveDialog;
                }

                _ => {}
            },

            // ── Export pick ───────────────────────────────────────────────────
            Screen::ExportPick => match key.code {
                KeyCode::Esc => app.screen = Screen::Browser,
                KeyCode::Up | KeyCode::Char('k') => app.load_cursor_up(),
                KeyCode::Down | KeyCode::Char('j') => app.load_cursor_down(),
                KeyCode::Enter => app.confirm_export(),
                _ => {}
            },

            // ── Result log ────────────────────────────────────────────────────
            Screen::ResultLog => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => app.screen = Screen::Browser,
                KeyCode::Up | KeyCode::Char('k') => app.log_scroll_up(),
                KeyCode::Down | KeyCode::Char('j') => {
                    app.log_scroll_down(app.result_log.len());
                }
                _ => {}
            },

            // ── Diff pick ─────────────────────────────────────────────────────
            Screen::DiffPick => match key.code {
                KeyCode::Esc => {
                    app.diff_pick_step = 0;
                    app.diff_profile_a = None;
                    app.screen = Screen::Browser;
                }
                KeyCode::Up | KeyCode::Char('k') => app.load_cursor_up(),
                KeyCode::Down | KeyCode::Char('j') => app.load_cursor_down(),
                KeyCode::Enter => app.diff_select(),
                _ => {}
            },

            // ── Diff result ───────────────────────────────────────────────────
            Screen::DiffView => match key.code {
                KeyCode::Esc | KeyCode::Char('b') | KeyCode::Char('B') | KeyCode::Char('q') => {
                    app.screen = Screen::Browser
                }
                _ => {}
            },

            // ── Popup ─────────────────────────────────────────────────────────
            Screen::Popup => {
                app.screen = Screen::Browser;
            }
        }
    }
    Ok(())
}
