use super::registry;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io::{self, Write};
use std::time::Duration;

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AddonOpt {
    pub name: &'static str,
    pub description: &'static str,
    pub selected: bool,
}

fn all_addon_opts() -> Vec<AddonOpt> {
    let reg = registry();
    reg.into_iter()
        .map(|a| AddonOpt {
            name: a.name,
            description: a.description,
            selected: !a.installed,
        })
        .collect()
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn run() -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let addons = select_addons(&mut terminal)?;

    // Exit TUI mode
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;

    let result = if addons.is_empty() {
        println!("No add-ons selected.");
        Ok(())
    } else {
        install_selected(&addons)
    };

    result
}

/// Interactive TUI to select add-ons.
/// Returns the list of selected add-on names (empty = cancelled).
fn select_addons(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<Vec<&'static str>> {
    let mut addons = all_addon_opts();
    let mut cursor: usize = 0;

    loop {
        terminal.draw(|f| render_select(f, &addons, cursor))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Up => {
                        cursor = cursor.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        cursor = (cursor + 1).min(addons.len().saturating_sub(1));
                    }
                    KeyCode::Char(' ') => {
                        addons[cursor].selected = !addons[cursor].selected;
                    }
                    KeyCode::Enter => {
                        let selected: Vec<&'static str> = addons
                            .iter()
                            .filter(|a| a.selected)
                            .map(|a| a.name)
                            .collect();
                        if selected.is_empty() {
                            return Ok(vec![]);
                        }
                        return Ok(selected);
                    }
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(vec![]),
                    _ => {}
                }
            }
        }
    }
}

/// Install selected add-ons sequentially with progress output.
fn install_selected(names: &[&'static str]) -> Result<()> {
    println!();
    println!("{}", "─".repeat(60));
    println!(" Installing {} add-on(s)...", names.len());
    println!();

    let mut success = 0;
    let mut failed = 0;

    for name in names {
        print!("  {:<30} ", name);
        io::stdout().flush()?;

        match super::install(name) {
            Ok(()) => {
                println!("✓");
                success += 1;
            }
            Err(e) => {
                println!("✗");
                eprintln!("    error: {e}");
                failed += 1;
            }
        }
    }

    println!();
    println!(" {} installed, {} failed", success, failed);
    println!("{}", "─".repeat(60));
    println!();
    println!("  Reload or restart the respective tools to activate.");

    Ok(())
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render_select(f: &mut ratatui::Frame, addons: &[AddonOpt], cursor: usize) {
    let area = f.area();
    let block = Block::default()
        .title(" tlink · install add-ons ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Fill(1)])
        .split(inner)[0];

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "Select add-ons to install:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, a) in addons.iter().enumerate() {
        let on_cursor = i == cursor;
        let prefix = if on_cursor { "❯ " } else { "  " };
        let check = if a.selected {
            Span::styled(
                "[x] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
        };
        let name_sty = if on_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let desc_sty = if on_cursor {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::raw(prefix),
            check,
            Span::styled(a.name, name_sty),
            Span::raw("  "),
            Span::styled(a.description, desc_sty),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑/↓ move  •  Space toggle  •  Enter install  •  q quit",
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(lines), content);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_addon_opts_exist() {
        let opts = all_addon_opts();
        assert!(!opts.is_empty(), "should have at least one addon");
        let names: Vec<&str> = opts.iter().map(|a| a.name).collect();
        assert!(names.contains(&"claude-notification"));
    }

    #[test]
    fn test_new_addons_selected_by_default() {
        let opts = all_addon_opts();
        // codex-notification and gemini-notification should always be selected
        // since they're newly added and never installed before
        for a in &opts {
            if a.name == "codex-notification" || a.name == "gemini-notification" {
                assert!(a.selected, "{} should be selected by default", a.name);
            }
        }
    }
}
