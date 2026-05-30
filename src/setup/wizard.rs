use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::{io, time::Duration};

#[derive(Debug, PartialEq, Clone)]
pub enum WizardState {
    Welcome,
    SelectTerminal {
        terminals: Vec<String>,
        selected: usize,
    },
    Confirm {
        terminal: String,
    },
    TelemetryPrompt {
        terminal: String,
    },
    Installing {
        terminal_name: String,
    },
    Verify {
        terminal_name: String,
        success: bool,
    },
    Done {
        terminal: String,
    },
    Cancelled,
}

pub const KNOWN_TERMINALS: &[&str] = &["iTerm2", "Ghostty", "Kitty", "WezTerm", "Terminal"];

pub fn detect_terminals() -> Vec<String> {
    KNOWN_TERMINALS
        .iter()
        .filter(|&&name| {
            std::path::Path::new(&format!("/Applications/{}.app", name)).exists()
                || std::path::Path::new(&format!("/System/Applications/Utilities/{}.app", name))
                    .exists()
        })
        .map(|s| s.to_string())
        .collect()
}

pub fn next_state(state: WizardState, key: KeyCode) -> WizardState {
    match (state, key) {
        (WizardState::Welcome, KeyCode::Enter) => {
            let mut terminals = detect_terminals();
            if terminals.is_empty() {
                terminals = KNOWN_TERMINALS.iter().map(|s| s.to_string()).collect();
            }
            WizardState::SelectTerminal {
                terminals,
                selected: 0,
            }
        }
        (WizardState::Welcome, KeyCode::Char('q') | KeyCode::Esc) => WizardState::Cancelled,

        (
            WizardState::SelectTerminal {
                terminals,
                selected,
            },
            KeyCode::Up,
        ) => WizardState::SelectTerminal {
            terminals,
            selected: selected.saturating_sub(1),
        },
        (
            WizardState::SelectTerminal {
                terminals,
                selected,
            },
            KeyCode::Down,
        ) => {
            let max = terminals.len().saturating_sub(1);
            WizardState::SelectTerminal {
                terminals,
                selected: (selected + 1).min(max),
            }
        }
        (
            WizardState::SelectTerminal {
                terminals,
                selected,
            },
            KeyCode::Enter,
        ) => WizardState::Confirm {
            terminal: terminals[selected].clone(),
        },
        (WizardState::SelectTerminal { .. }, KeyCode::Char('q') | KeyCode::Esc) => {
            WizardState::Cancelled
        }

        (WizardState::Confirm { terminal }, KeyCode::Enter | KeyCode::Char('y')) => {
            // Skip telemetry prompt if user already decided
            let telemetry_decided = crate::config::load()
                .ok()
                .and_then(|c| c.telemetry_enabled)
                .is_some();
            if telemetry_decided {
                WizardState::Installing {
                    terminal_name: terminal,
                }
            } else {
                WizardState::TelemetryPrompt { terminal }
            }
        }
        (WizardState::Confirm { .. }, KeyCode::Char('n') | KeyCode::Esc) => WizardState::Cancelled,

        (WizardState::TelemetryPrompt { terminal }, KeyCode::Enter | KeyCode::Char('y')) => {
            WizardState::Installing {
                terminal_name: terminal,
            }
        }
        (WizardState::TelemetryPrompt { terminal }, KeyCode::Char('n')) => {
            WizardState::Installing {
                terminal_name: terminal,
            }
        }
        (WizardState::TelemetryPrompt { .. }, KeyCode::Esc) => WizardState::Cancelled,

        (WizardState::Verify { terminal_name, .. }, KeyCode::Enter) => WizardState::Done {
            terminal: terminal_name,
        },

        (state, _) => state,
    }
}

fn verify_scheme() -> bool {
    std::process::Command::new(
        "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister"
    )
    .args(["-dump"])
    .output()
    .map(|o| String::from_utf8_lossy(&o.stdout).contains("tmux"))
    .unwrap_or(false)
}

pub fn run_wizard() -> Result<Option<String>> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal);

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Option<String>> {
    let mut state = WizardState::Welcome;
    // Track telemetry choice during the wizard (set in next_state transition)
    let mut telemetry_opt_in: Option<bool> = None;

    loop {
        terminal.draw(|f| render(f, &state))?;

        match state.clone() {
            WizardState::Installing { terminal_name } => {
                // Apply telemetry choice before installing
                match telemetry_opt_in {
                    Some(true) => {
                        let mut cfg = crate::config::load().unwrap_or_default();
                        cfg.telemetry_enabled = Some(true);
                        let _ = crate::config::save(&cfg);
                    }
                    Some(false) => {
                        let mut cfg = crate::config::load().unwrap_or_default();
                        cfg.telemetry_enabled = Some(false);
                        let _ = crate::config::save(&cfg);
                    }
                    None => {}
                }
                let config = crate::config::Config {
                    terminal: Some(terminal_name.clone()),
                    ..Default::default()
                };
                let _ = crate::config::save(&config);
                let install_ok = crate::bundle::create().is_ok();
                let success = install_ok && verify_scheme();
                state = WizardState::Verify {
                    terminal_name,
                    success,
                };
                continue;
            }
            WizardState::Cancelled => return Ok(None),
            WizardState::Done { terminal } => return Ok(Some(terminal)),
            _ => {}
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                // Capture telemetry choice before transitioning
                if matches!(state, WizardState::TelemetryPrompt { .. }) {
                    match code {
                        KeyCode::Enter | KeyCode::Char('y') => {
                            telemetry_opt_in = Some(true);
                        }
                        KeyCode::Char('n') => {
                            telemetry_opt_in = Some(false);
                        }
                        _ => {}
                    }
                }
                state = next_state(state, code);
            }
        }
    }
}

fn render(f: &mut ratatui::Frame, state: &WizardState) {
    let area = f.area();
    let block = Block::default()
        .title(" tlink setup ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Fill(1)])
        .split(inner)[0];

    match state {
        WizardState::Welcome => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Welcome to tlink setup",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("This wizard will:"),
                    Line::from("  1. Select your terminal emulator"),
                    Line::from("  2. Compile and register the tmux:// URI handler"),
                    Line::from("  3. Verify the setup works"),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Enter to continue  •  q to quit",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }
        WizardState::SelectTerminal {
            terminals,
            selected,
        } => {
            let items: Vec<ListItem> = terminals
                .iter()
                .map(|t| ListItem::new(t.as_str()))
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .title("Select your terminal  (↑/↓ move  •  Enter select  •  q quit)"),
                )
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            let mut list_state = ListState::default();
            list_state.select(Some(*selected));
            f.render_stateful_widget(list, content, &mut list_state);
        }
        WizardState::Confirm { terminal } => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(format!("Terminal: {terminal}")),
                    Line::from(""),
                    Line::from("Will create:   ~/Applications/TmuxLink.app"),
                    Line::from("Will register: tmux:// URI scheme"),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Enter/y to confirm  •  n/Esc to cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }
        WizardState::TelemetryPrompt { .. } => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Help improve tlink?",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("Share anonymous usage data to help improve tlink."),
                    Line::from("No personal info collected. See tlink telemetry status."),
                    Line::from(""),
                    Line::from(Span::styled(
                        "y/Enter: enable  •  n: skip  •  Esc: cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }
        WizardState::Installing { .. } => {
            f.render_widget(
                Paragraph::new("Compiling handler and registering tmux:// scheme...")
                    .alignment(Alignment::Center),
                content,
            );
        }
        WizardState::Verify { success, .. } => {
            let (msg, color) = if *success {
                ("✓ Verification passed", Color::Green)
            } else {
                (
                    "! Verification inconclusive — handler may activate after relogin",
                    Color::Yellow,
                )
            };
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(msg, Style::default().fg(color))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Enter to finish",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }
        WizardState::Done { terminal } => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Setup complete!",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!("Terminal: {terminal}")),
                    Line::from(""),
                    Line::from("Try it:  open tmux://your-session-name"),
                    Line::from("Docs:    tlink --help"),
                ]),
                content,
            );
        }
        WizardState::Cancelled => {
            f.render_widget(Paragraph::new("Setup cancelled."), content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_enter_transitions_to_select() {
        let next = next_state(WizardState::Welcome, KeyCode::Enter);
        assert!(matches!(next, WizardState::SelectTerminal { .. }));
    }

    #[test]
    fn test_welcome_q_cancels() {
        assert_eq!(
            next_state(WizardState::Welcome, KeyCode::Char('q')),
            WizardState::Cancelled
        );
        assert_eq!(
            next_state(WizardState::Welcome, KeyCode::Esc),
            WizardState::Cancelled
        );
    }

    #[test]
    fn test_select_down_moves_cursor() {
        let state = WizardState::SelectTerminal {
            terminals: vec!["iTerm2".into(), "Ghostty".into()],
            selected: 0,
        };
        let next = next_state(state, KeyCode::Down);
        assert!(matches!(
            next,
            WizardState::SelectTerminal { selected: 1, .. }
        ));
    }

    #[test]
    fn test_select_down_clamps_at_last() {
        let state = WizardState::SelectTerminal {
            terminals: vec!["iTerm2".into()],
            selected: 0,
        };
        let next = next_state(state, KeyCode::Down);
        assert!(matches!(
            next,
            WizardState::SelectTerminal { selected: 0, .. }
        ));
    }

    #[test]
    fn test_select_up_clamps_at_zero() {
        let state = WizardState::SelectTerminal {
            terminals: vec!["iTerm2".into(), "Ghostty".into()],
            selected: 0,
        };
        let next = next_state(state, KeyCode::Up);
        assert!(matches!(
            next,
            WizardState::SelectTerminal { selected: 0, .. }
        ));
    }

    #[test]
    fn test_select_enter_picks_selected_terminal() {
        let state = WizardState::SelectTerminal {
            terminals: vec!["iTerm2".into(), "Ghostty".into()],
            selected: 1,
        };
        assert_eq!(
            next_state(state, KeyCode::Enter),
            WizardState::Confirm {
                terminal: "Ghostty".into()
            }
        );
    }

    #[test]
    fn test_confirm_enter_goes_to_telemetry_prompt() {
        let state = WizardState::Confirm {
            terminal: "iTerm2".into(),
        };
        assert_eq!(
            next_state(state, KeyCode::Enter),
            WizardState::TelemetryPrompt {
                terminal: "iTerm2".into()
            }
        );
    }

    #[test]
    fn test_telemetry_y_goes_to_installing() {
        let state = WizardState::TelemetryPrompt {
            terminal: "Ghostty".into(),
        };
        let next = next_state(state, KeyCode::Char('y'));
        assert_eq!(
            next,
            WizardState::Installing {
                terminal_name: "Ghostty".into()
            }
        );
    }

    #[test]
    fn test_telemetry_n_goes_to_installing() {
        let state = WizardState::TelemetryPrompt {
            terminal: "Ghostty".into(),
        };
        let next = next_state(state, KeyCode::Char('n'));
        assert_eq!(
            next,
            WizardState::Installing {
                terminal_name: "Ghostty".into()
            }
        );
    }

    #[test]
    fn test_telemetry_esc_cancels() {
        let state = WizardState::TelemetryPrompt {
            terminal: "iTerm2".into(),
        };
        assert_eq!(next_state(state, KeyCode::Esc), WizardState::Cancelled);
    }

    #[test]
    fn test_confirm_n_cancels() {
        let state = WizardState::Confirm {
            terminal: "iTerm2".into(),
        };
        assert_eq!(
            next_state(state, KeyCode::Char('n')),
            WizardState::Cancelled
        );
    }

    #[test]
    fn test_verify_enter_goes_to_done() {
        let state = WizardState::Verify {
            terminal_name: "iTerm2".into(),
            success: true,
        };
        assert_eq!(
            next_state(state, KeyCode::Enter),
            WizardState::Done {
                terminal: "iTerm2".into()
            }
        );
    }

    #[test]
    fn test_detect_terminals_does_not_panic() {
        let _ = detect_terminals();
    }
}
