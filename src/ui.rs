// src/ui.rs — ratatui rendering

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Screen};
use crate::library::human_bytes;
use crate::profile::Profile;

pub fn draw(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Browser        => draw_browser(f, app),
        Screen::SaveDialog     => { draw_browser(f, app); draw_save_dialog(f, app); }
        Screen::LoadDialog     => draw_load_dialog(f, app, false),
        Screen::ProfileEditor  => draw_profile_editor(f, app),
        Screen::DiffPick       => draw_load_dialog(f, app, true),
        Screen::DiffView       => draw_diff(f, app),
        Screen::ExportPick     => draw_export_pick(f, app),
        Screen::ResultLog      => draw_result_log(f, app),
        Screen::Popup          => { draw_browser(f, app); draw_popup(f, app); }
    }
}

// ─── Browser ─────────────────────────────────────────────────────────────────

fn draw_browser(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title bar
            Constraint::Min(5),     // main list
            Constraint::Length(4),  // status / summary
            Constraint::Length(2),  // keybinds
        ])
        .split(area);

    // Title
    let title = Paragraph::new(format!(
        " 🎵  Soundfont Manager  ·  Library: {}  ·  Profile: \"{}\"",
        app.library_path.display(),
        app.profile_name,
    ))
    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Font list — show slot number if this font is in the ordered profile
    let items: Vec<ListItem> = app.library.iter().enumerate().map(|(i, font)| {
        let slot = app.profile_order.iter().position(|n| n == &font.name);
        let checked = if slot.is_some() { "◉" } else { "○" };
        let slot_label = match slot {
            Some(idx) => format!("{:>3}. ", idx + 1),
            None      => "     ".to_string(),
        };
        let style = if i == app.cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if app.selected.contains(&font.name) {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let line = Line::from(vec![
            Span::styled(format!(" {checked} "), style),
            Span::styled(slot_label, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:<35}", font.name), style),
            Span::styled(
                format!("{:>10}  {:>5} files", font.size_human(), font.file_count),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        ListItem::new(line)
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.cursor));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Soundfonts (Space to toggle) "))
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    // Summary bar
    let total_lib: u64 = app.library.iter().map(|f| f.size_bytes).sum();
    let summary = format!(
        " Selected: {}  ({} fonts / {} files)    Library total: {}  ({} fonts)",
        app.selected_size_human(),
        app.selected.len(),
        app.total_selected_files(),
        human_bytes(total_lib),
        app.library.len(),
    );
    let status = Paragraph::new(summary)
        .style(Style::default().fg(Color::Magenta))
        .block(Block::default().borders(Borders::ALL).title(" Summary "));
    f.render_widget(status, chunks[2]);

    // Keybinds
    let keys = Paragraph::new(
        " [Space] Toggle  [E] Edit Order  [S] Save  [L] Load  [D] Diff  [X] Export  [A] All  [C] Clear  [Q] Quit"
    )
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(keys, chunks[3]);
}

// ─── Save dialog ─────────────────────────────────────────────────────────────

fn draw_save_dialog(f: &mut Frame, app: &App) {
    let popup = centered_rect(50, 7, f.area());
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Save Profile ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(inner);

    f.render_widget(
        Paragraph::new("Profile name:").style(Style::default().fg(Color::Gray)),
        layout[0],
    );
    f.render_widget(
        Paragraph::new(format!("> {}_", app.input_buffer))
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        layout[1],
    );
    f.render_widget(
        Paragraph::new("[Enter] Save  [Esc] Cancel").style(Style::default().fg(Color::DarkGray)),
        layout[2],
    );
}

// ─── Load dialog ─────────────────────────────────────────────────────────────

pub fn draw_load_dialog(f: &mut Frame, app: &App, diff_mode: bool) {
    let area = f.area();
    let title = if diff_mode {
        if app.diff_pick_step == 0 { " Diff — Select Profile A " } else { " Diff — Select Profile B " }
    } else {
        " Load Profile "
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let items: Vec<ListItem> = app.saved_profiles.iter().enumerate().map(|(i, path)| {
        let name = path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        let style = if i == app.load_cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let detail = Profile::load_meta(path)
            .map(|count| format!("  {:>3} fonts", count))
            .unwrap_or_default();

        let line = Line::from(vec![
            Span::styled(format!(" {:<35}", name), style),
            Span::styled(detail, Style::default().fg(Color::DarkGray)),
        ]);
        ListItem::new(line)
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.load_cursor));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    let hint = if diff_mode {
        " [Enter] Select  [Esc] Cancel"
    } else {
        " [Enter] Load → opens editor  [Esc] Cancel"
    };
    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
}

// ─── Profile editor ───────────────────────────────────────────────────────────

fn draw_profile_editor(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // title
            Constraint::Min(5),      // ordered list
            Constraint::Length(4),   // summary
            Constraint::Length(2),   // keybinds
        ])
        .split(area);

    let dragging = app.editor_drag.is_some();
    let title_str = if dragging {
        format!(
            " ✏  Profile Editor  ·  \"{}\"  ·  MOVING item — [Space] Drop  [↑/↓] Reposition ",
            app.profile_name
        )
    } else {
        format!(
            " ✏  Profile Editor  ·  \"{}\"  ·  {} fonts ",
            app.profile_name,
            app.profile_order.len()
        )
    };
    let title = Paragraph::new(title_str)
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Build ordered list
    let items: Vec<ListItem> = app.profile_order.iter().enumerate().map(|(idx, name)| {
        let is_cursor   = idx == app.editor_cursor;
        let is_dragging = app.editor_drag == Some(idx);

        let slot_str = format!("{:>3}. ", idx + 1);

        // Look up size info from library
        let detail = app.library.iter().find(|f| &f.name == name)
            .map(|f| format!("  {:>10}  {:>5} files", f.size_human(), f.file_count))
            .unwrap_or_default();

        let (row_style, prefix) = if is_dragging {
            (Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD), "⠿ ")
        } else if is_cursor {
            (Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD), "▶ ")
        } else {
            (Style::default().fg(Color::Green), "  ")
        };

        let line = Line::from(vec![
            Span::styled(prefix.to_string(), row_style),
            Span::styled(slot_str, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:<40}", name), row_style),
            Span::styled(detail, Style::default().fg(if is_dragging { Color::DarkGray } else { Color::DarkGray })),
        ]);
        ListItem::new(line)
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.editor_cursor));

    let list_title = if dragging {
        " Drag to reorder — release with [Space] "
    } else {
        " Ordered fonts (slot = export folder prefix) "
    };

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(list_title)
            .border_style(if dragging {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            }))
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    // Summary
    let total_size = app.selected_size_human();
    let total_files = app.total_selected_files();
    let summary = format!(
        " Profile: \"{}\"  ·  {} fonts  ·  {} files  ·  {}",
        app.profile_name,
        app.profile_order.len(),
        total_files,
        total_size,
    );
    f.render_widget(
        Paragraph::new(summary)
            .style(Style::default().fg(Color::Magenta))
            .block(Block::default().borders(Borders::ALL).title(" Summary ")),
        chunks[2],
    );

    // Keybinds
    f.render_widget(
        Paragraph::new(
            " [↑/↓] Move cursor  [Space] Grab/Drop  [Del] Remove  [S] Save  [Esc] Back to browser"
        )
        .style(Style::default().fg(Color::DarkGray)),
        chunks[3],
    );
}

// ─── Diff view ───────────────────────────────────────────────────────────────

fn draw_diff(f: &mut Frame, app: &App) {
    let area = f.area();
    let d = match &app.diff_result {
        Some(d) => d,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    // Title
    let title = Paragraph::new(format!(
        " Diff: \"{}\"  vs  \"{}\"",
        d.name_a, d.name_b
    ))
    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Stats comparison
    let stats_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(24),
            Constraint::Percentage(38),
        ])
        .split(chunks[1]);

    let count_a = d.diff.in_both.len() + d.diff.only_in_a.len();
    let count_b = d.diff.in_both.len() + d.diff.only_in_b.len();

    let stat_a = Paragraph::new(format!(
        " Fonts : {}\n Files : {}\n Size  : {}",
        count_a, d.files_a, human_bytes(d.size_a),
    ))
    .style(Style::default().fg(Color::Green))
    .block(Block::default().borders(Borders::ALL).title(format!(" {} ", d.name_a)));
    f.render_widget(stat_a, stats_chunks[0]);

    let font_delta = count_b as i64 - count_a as i64;
    let file_delta = d.files_b as i64 - d.files_a as i64;
    let size_delta = d.size_b as i64 - d.size_a as i64;

    let fmt_delta_i = |v: i64| if v > 0 { format!("+{v}") } else { format!("{v}") };
    let fmt_delta_bytes = |v: i64| {
        let abs = human_bytes(v.unsigned_abs());
        if v > 0 { format!("+{abs}") } else if v < 0 { format!("-{abs}") } else { "=".into() }
    };
    let delta_color = |v: i64| if v > 0 { Color::LightBlue } else if v < 0 { Color::LightRed } else { Color::DarkGray };

    let delta_text = vec![
        Line::from(vec![
            Span::raw(" Fonts : "),
            Span::styled(fmt_delta_i(font_delta), Style::default().fg(delta_color(font_delta))),
        ]),
        Line::from(vec![
            Span::raw(" Files : "),
            Span::styled(fmt_delta_i(file_delta), Style::default().fg(delta_color(file_delta))),
        ]),
        Line::from(vec![
            Span::raw(" Size  : "),
            Span::styled(fmt_delta_bytes(size_delta), Style::default().fg(delta_color(size_delta))),
        ]),
    ];
    let delta_panel = Paragraph::new(delta_text)
        .block(Block::default().borders(Borders::ALL).title(" Δ B − A "));
    f.render_widget(delta_panel, stats_chunks[1]);

    let stat_b = Paragraph::new(format!(
        " Fonts : {}\n Files : {}\n Size  : {}",
        count_b, d.files_b, human_bytes(d.size_b),
    ))
    .style(Style::default().fg(Color::Blue))
    .block(Block::default().borders(Borders::ALL).title(format!(" {} ", d.name_b)));
    f.render_widget(stat_b, stats_chunks[2]);

    // Font lists
    let list_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(chunks[2]);

    render_diff_list(f, list_chunks[0], &d.diff.only_in_a,
        &format!(" Only in \"{}\" ({}) ", d.name_a, d.diff.only_in_a.len()), Color::Green);
    render_diff_list(f, list_chunks[1], &d.diff.in_both,
        &format!(" Shared ({}) ", d.diff.in_both.len()), Color::White);
    render_diff_list(f, list_chunks[2], &d.diff.only_in_b,
        &format!(" Only in \"{}\" ({}) ", d.name_b, d.diff.only_in_b.len()), Color::Blue);

    f.render_widget(
        Paragraph::new(" [Esc] / [B] Back to browser")
            .style(Style::default().fg(Color::DarkGray)),
        chunks[3],
    );
}

fn render_diff_list(f: &mut Frame, area: Rect, items: &[String], title: &str, color: Color) {
    let list_items: Vec<ListItem> = items.iter()
        .map(|name| ListItem::new(format!(" {name}")))
        .collect();
    let list = List::new(list_items)
        .style(Style::default().fg(color))
        .block(Block::default().borders(Borders::ALL).title(title.to_string()));
    f.render_widget(list, area);
}

// ─── Export pick ──────────────────────────────────────────────────────────────

fn draw_export_pick(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    let items: Vec<ListItem> = app.saved_profiles.iter().enumerate().map(|(i, path)| {
        let name = path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        let style = if i == app.load_cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let count = Profile::load_meta(path)
            .map(|c| format!("  {c} fonts"))
            .unwrap_or_default();
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {:<35}", name), style),
            Span::styled(count, Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.load_cursor));
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL)
            .title(" Export Profile — folders will be prefixed 1-NAME, 2-NAME … "))
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    let dest = app.library_path.parent()
        .unwrap_or(&app.library_path)
        .join("profiles")
        .display()
        .to_string();
    f.render_widget(
        Paragraph::new(format!(" Copies to: {dest}/<profile_name>/    [Enter] Export  [Esc] Cancel"))
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL)),
        chunks[1],
    );
}

// ─── Result log ───────────────────────────────────────────────────────────────

fn draw_result_log(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let visible_lines: Vec<ListItem> = app.result_log.iter()
        .skip(app.log_scroll)
        .map(|line| {
            let style = if line.starts_with('✓') {
                Style::default().fg(Color::Green)
            } else if line.starts_with("SKIP") {
                Style::default().fg(Color::Yellow)
            } else if line.contains('→') {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            };
            ListItem::new(format!(" {line}")).style(style)
        })
        .collect();

    let list = List::new(visible_lines)
        .block(Block::default().borders(Borders::ALL)
            .title(format!(" Result  ({} lines) ", app.result_log.len())));
    f.render_widget(list, chunks[0]);

    f.render_widget(
        Paragraph::new(" [↑/↓] Scroll  [Esc] Back to browser")
            .style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
}

// ─── Popup ───────────────────────────────────────────────────────────────────

fn draw_popup(f: &mut Frame, app: &App) {
    let popup = centered_rect(60, 5, f.area());
    f.render_widget(Clear, popup);
    let p = Paragraph::new(app.popup_message.as_str())
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Info ").style(Style::default().fg(Color::Yellow)));
    f.render_widget(p, popup);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height.min(100)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
