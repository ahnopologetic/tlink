use super::{InstallOptions, NotifMethod};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::{io, time::Duration};

// ── State machine ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum State {
    Welcome,
    SelectMethod {
        methods: Vec<NotifMethod>,
        cursor: usize,
    },
    Confirm {
        method: NotifMethod,
    },
    Installing {
        method: NotifMethod,
    },
    Done {
        method: NotifMethod,
    },
    Cancelled,
}

pub fn next_state(state: State, key: KeyCode) -> State {
    match (state, key) {
        // Welcome
        (State::Welcome, KeyCode::Enter) => {
            let methods = NotifMethod::platform_methods();
            let rec = NotifMethod::recommended_method();
            let cursor = methods.iter().position(|m| m == &rec).unwrap_or(0);
            State::SelectMethod { methods, cursor }
        }
        (State::Welcome, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // SelectMethod
        (State::SelectMethod { methods, cursor }, KeyCode::Up) => State::SelectMethod {
            methods,
            cursor: cursor.saturating_sub(1),
        },
        (State::SelectMethod { methods, cursor }, KeyCode::Down) => {
            let max = methods.len().saturating_sub(1);
            State::SelectMethod {
                methods,
                cursor: (cursor + 1).min(max),
            }
        }
        (
            State::SelectMethod {
                methods: _,
                cursor: _,
            },
            KeyCode::Enter,
        ) => State::Confirm {
            method: NotifMethod::Osascript,
        },
        (State::SelectMethod { .. }, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // Confirm
        (State::Confirm { method }, KeyCode::Enter | KeyCode::Char('y')) => {
            State::Installing { method }
        }
        (State::Confirm { .. }, KeyCode::Char('n') | KeyCode::Esc) => State::Cancelled,

        (s, _) => s,
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn run() -> Result<Option<InstallOptions>> {
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

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<Option<InstallOptions>> {
    let mut state = State::Welcome;
    let mut pending_method: Option<NotifMethod> = None;

    loop {
        terminal.draw(|f| render(f, &state))?;

        match state.clone() {
            State::Installing { method } => {
                let opts = InstallOptions {
                    method: method.clone(),
                };
                super::install_with_options(&opts)?;
                state = State::Done { method };
                continue;
            }
            State::Cancelled => return Ok(None),
            _ => {}
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if let State::Done { ref method, .. } = state {
                    return Ok(Some(InstallOptions {
                        method: method.clone(),
                    }));
                }

                if let State::SelectMethod {
                    ref methods,
                    cursor,
                } = state
                {
                    if code == KeyCode::Enter {
                        pending_method = Some(methods[cursor].clone());
                    }
                }

                state = next_state(state, code);

                if let State::Confirm { ref mut method, .. } = state {
                    if let Some(m) = pending_method.take() {
                        *method = m;
                    }
                }
            }
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render(f: &mut ratatui::Frame, state: &State) {
    let area = f.area();
    let block = Block::default()
        .title(" tlink · install codex-notification ")
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
        State::Welcome => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "codex-notification add-on",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("When Codex CLI finishes a task, you'll get"),
                    Line::from(
                        "a desktop notification showing the tmux location (session:window.pane).",
                    ),
                    Line::from(""),
                    Line::from("This wizard will configure:"),
                    Line::from("  • Notification delivery method"),
                    Line::from(""),
                    Line::from("Codex CLI uses the `notify` config option in"),
                    Line::from("  ~/.codex/config.toml"),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Enter to continue  •  q to quit",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }

        State::SelectMethod { methods, cursor } => {
            let rec = NotifMethod::recommended_method();
            let items: Vec<ListItem> = methods
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let selected = i == *cursor;
                    let is_rec = m == &rec;
                    let prefix = if selected { "❯ " } else { "  " };
                    let name_style = if selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let tag = if is_rec {
                        if m.available() {
                            Span::styled(
                                "[★ recommended]",
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            Span::styled(
                                "[★ recommended — needs install]",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )
                        }
                    } else if m.available() {
                        Span::styled("[ok]", Style::default().fg(Color::Green))
                    } else {
                        Span::styled("[--]", Style::default().fg(Color::DarkGray))
                    };
                    let desc_style = if selected {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::raw(prefix),
                            Span::styled(m.label(), name_style),
                            Span::raw("  "),
                            tag,
                        ]),
                        Line::from(Span::styled(format!("    {}", m.description()), desc_style)),
                        Line::from(""),
                    ])
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title("Notification method  (↑/↓ move  •  Enter select  •  q quit)"),
                )
                .highlight_style(Style::default());
            let mut ls = ListState::default();
            ls.select(Some(*cursor));
            f.render_stateful_widget(list, content, &mut ls);
        }

        State::Confirm { method } => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Review",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!("  Notification method : {}", method.label())),
                    Line::from(
                        "  Hook script         : ~/.config/tlink/hooks/codex-notification.sh"
                            .to_string(),
                    ),
                    Line::from("  Codex config        : ~/.codex/config.toml".to_string()),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Enter/y to install  •  n/Esc to cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }

        State::Installing { .. } => {
            f.render_widget(Paragraph::new("Installing…"), content);
        }

        State::Done { method } => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "Installed!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("  Method : {}", method.label())),
                Line::from(""),
            ];
            if *method == NotifMethod::Osascript || *method == NotifMethod::NotifySend {
                lines.push(Line::from(Span::styled(
                    "  Tip: install terminal-notifier (macOS) or dunst (Linux) for banner notifications",
                    Style::default().fg(Color::Yellow),
                )));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                "Press Enter to exit",
                Style::default().fg(Color::DarkGray),
            )));
            f.render_widget(Paragraph::new(lines), content);
        }

        State::Cancelled => {
            f.render_widget(Paragraph::new("Installation cancelled."), content);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn method_state() -> State {
        State::SelectMethod {
            methods: vec![NotifMethod::TerminalNotifier, NotifMethod::Osascript],
            cursor: 0,
        }
    }

    #[test]
    fn test_welcome_enter_goes_to_select_method() {
        assert!(matches!(
            next_state(State::Welcome, KeyCode::Enter),
            State::SelectMethod { .. }
        ));
    }

    #[test]
    fn test_welcome_q_cancels() {
        assert!(matches!(
            next_state(State::Welcome, KeyCode::Char('q')),
            State::Cancelled
        ));
    }

    #[test]
    fn test_select_method_down_moves_cursor() {
        let next = next_state(method_state(), KeyCode::Down);
        assert!(matches!(next, State::SelectMethod { cursor: 1, .. }));
    }

    #[test]
    fn test_select_method_enter_goes_to_confirm() {
        assert!(matches!(
            next_state(method_state(), KeyCode::Enter),
            State::Confirm { .. }
        ));
    }

    #[test]
    fn test_confirm_enter_goes_to_installing() {
        let state = State::Confirm {
            method: NotifMethod::Osascript,
        };
        assert!(matches!(
            next_state(state, KeyCode::Enter),
            State::Installing { .. }
        ));
    }

    #[test]
    fn test_confirm_n_cancels() {
        let state = State::Confirm {
            method: NotifMethod::Osascript,
        };
        assert!(matches!(
            next_state(state, KeyCode::Char('n')),
            State::Cancelled
        ));
    }
}
