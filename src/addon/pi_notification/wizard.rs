use super::{InstallOptions, PiEvent, PI_CATEGORIES};
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
use std::{io, time::Duration};

// ── State machine ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub enum State {
    Welcome,
    SelectEvents {
        events: Vec<EventOpt>,
        cursor: usize,
        search: String,
        active_tab: usize,
    },
    Confirm {
        events: Vec<PiEvent>,
    },
    Installing {
        events: Vec<PiEvent>,
    },
    Done {
        events: Vec<PiEvent>,
    },
    Cancelled,
}

#[derive(Clone)]
pub struct EventOpt {
    pub event: PiEvent,
    pub selected: bool,
}

impl EventOpt {
    fn all_events() -> Vec<Self> {
        use PiEvent::*;
        let defs: &[(PiEvent, bool)] = &[
            // Agent
            (AgentEnd, true),
            (TurnEnd, false),
            // Session
            (SessionStart, false),
            (SessionShutdown, false),
            // Tools
            (ToolExecutionEnd, false),
        ];
        defs.iter()
            .map(|(e, s)| Self {
                event: e.clone(),
                selected: *s,
            })
            .collect()
    }
}

/// Returns the indices into `events` that are visible given the current tab and search query.
pub fn visible_indices(events: &[EventOpt], search: &str, active_tab: usize) -> Vec<usize> {
    let q = search.to_lowercase();
    events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let tab_match =
                active_tab == 0 || PI_CATEGORIES.get(active_tab - 1) == Some(&e.event.category());
            let search_match = q.is_empty()
                || e.event.label().to_lowercase().contains(&q)
                || e.event.description().to_lowercase().contains(&q);
            if tab_match && search_match {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

pub fn next_state(state: State, key: KeyCode) -> State {
    match (state, key) {
        // Welcome
        (State::Welcome, KeyCode::Enter) => State::SelectEvents {
            events: EventOpt::all_events(),
            cursor: 0,
            search: String::new(),
            active_tab: 0,
        },
        (State::Welcome, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // SelectEvents
        (
            State::SelectEvents {
                events,
                cursor: _,
                search: _,
                active_tab,
            },
            KeyCode::Tab,
        ) => {
            let n_tabs = PI_CATEGORIES.len() + 1;
            let next_tab = (active_tab + 1) % n_tabs;
            State::SelectEvents {
                events,
                cursor: 0,
                search: String::new(),
                active_tab: next_tab,
            }
        }
        (
            State::SelectEvents {
                events,
                cursor,
                search,
                active_tab,
            },
            KeyCode::Up,
        ) => {
            let visible = visible_indices(&events, &search, active_tab);
            let pos = visible.iter().position(|&i| i == cursor).unwrap_or(0);
            let new_cursor = visible
                .get(pos.saturating_sub(1))
                .copied()
                .unwrap_or(cursor);
            State::SelectEvents {
                events,
                cursor: new_cursor,
                search,
                active_tab,
            }
        }
        (
            State::SelectEvents {
                events,
                cursor,
                search,
                active_tab,
            },
            KeyCode::Down,
        ) => {
            let visible = visible_indices(&events, &search, active_tab);
            let pos = visible.iter().position(|&i| i == cursor).unwrap_or(0);
            let new_cursor = visible
                .get((pos + 1).min(visible.len().saturating_sub(1)))
                .copied()
                .unwrap_or(cursor);
            State::SelectEvents {
                events,
                cursor: new_cursor,
                search,
                active_tab,
            }
        }
        (
            State::SelectEvents {
                mut events,
                cursor,
                search,
                active_tab,
            },
            KeyCode::Char(' '),
        ) => {
            events[cursor].selected = !events[cursor].selected;
            State::SelectEvents {
                events,
                cursor,
                search,
                active_tab,
            }
        }
        (
            State::SelectEvents {
                events,
                search,
                active_tab,
                ..
            },
            KeyCode::Enter,
        ) => {
            if !search.is_empty() {
                return State::SelectEvents {
                    events,
                    cursor: 0,
                    search: String::new(),
                    active_tab,
                };
            }
            let selected: Vec<PiEvent> = events
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.event.clone())
                .collect();
            let events_final = if selected.is_empty() {
                vec![PiEvent::AgentEnd]
            } else {
                selected
            };
            State::Confirm {
                events: events_final,
            }
        }
        (
            State::SelectEvents {
                events,
                mut search,
                active_tab,
                cursor: _,
            },
            KeyCode::Backspace,
        ) => {
            search.pop();
            let new_cursor = visible_indices(&events, &search, active_tab)
                .first()
                .copied()
                .unwrap_or(0);
            State::SelectEvents {
                events,
                cursor: new_cursor,
                search,
                active_tab,
            }
        }
        (
            State::SelectEvents {
                events,
                mut search,
                active_tab,
                cursor,
            },
            KeyCode::Char(c),
        ) => {
            search.push(c);
            let visible = visible_indices(&events, &search, active_tab);
            let new_cursor = if visible.contains(&cursor) {
                cursor
            } else {
                visible.first().copied().unwrap_or(cursor)
            };
            State::SelectEvents {
                events,
                cursor: new_cursor,
                search,
                active_tab,
            }
        }
        (State::SelectEvents { .. }, KeyCode::Esc) => State::Cancelled,

        // Confirm
        (State::Confirm { events }, KeyCode::Enter | KeyCode::Char('y')) => {
            State::Installing { events }
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

    loop {
        terminal.draw(|f| render(f, &state))?;

        match state.clone() {
            State::Installing { events } => {
                let opts = InstallOptions {
                    events: events.clone(),
                };
                super::install_with_options(&opts)?;
                state = State::Done { events };
                continue;
            }
            State::Cancelled => return Ok(None),
            _ => {}
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if let State::Done { ref events, .. } = state {
                    return Ok(Some(InstallOptions {
                        events: events.clone(),
                    }));
                }
                state = next_state(state, code);
            }
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render(f: &mut ratatui::Frame, state: &State) {
    let area = f.area();
    let block = Block::default()
        .title(" tlink · install pi-notification ")
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
                        "pi-notification add-on",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("When Pi finishes a turn or needs your attention, you'll get"),
                    Line::from(
                        "a desktop notification showing the tmux location (session:window.pane).",
                    ),
                    Line::from(""),
                    Line::from("This wizard will configure:"),
                    Line::from("  • Which Pi events trigger a notification"),
                    Line::from(""),
                    Line::from("Installed as a Pi extension at:"),
                    Line::from("  ~/.pi/agent/extensions/pi-notification.ts"),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Enter to continue  •  q to quit",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                content,
            );
        }

        State::SelectEvents {
            events,
            cursor,
            search,
            active_tab,
        } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Fill(1),
                    Constraint::Length(1),
                ])
                .split(content);

            // Tab bar
            let tab_spans: Vec<Span> = std::iter::once("All".to_string())
                .chain(PI_CATEGORIES.iter().map(|c| c.to_string()))
                .enumerate()
                .flat_map(|(i, label)| {
                    let sty = if i == *active_tab {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    [Span::styled(label, sty), Span::raw("  ")]
                })
                .collect();
            f.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[0]);

            // Event list
            let visible = visible_indices(events, search, *active_tab);
            let mut lines: Vec<Line> = Vec::new();
            let mut prev_cat = "";

            for &i in &visible {
                let e = &events[i];
                if *active_tab == 0 {
                    let cat = e.event.category();
                    if cat != prev_cat {
                        if !lines.is_empty() {
                            lines.push(Line::from(""));
                        }
                        lines.push(Line::from(Span::styled(
                            format!(" {} ", cat),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )));
                        prev_cat = cat;
                    }
                }
                let on_cursor = i == *cursor;
                let prefix = if on_cursor { "❯ " } else { "  " };
                let check = if e.selected {
                    Span::styled(
                        "[x] ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
                };
                let label_sty = if on_cursor {
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
                    Span::styled(e.event.label(), label_sty),
                    Span::raw("  "),
                    Span::styled(e.event.description(), desc_sty),
                ]));
            }

            if visible.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  no matches",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            f.render_widget(
                Paragraph::new(lines).block(Block::default().title(
                    "↑/↓ move  •  Space toggle  •  Tab switch category  •  type to search  •  Enter confirm",
                )),
                chunks[1],
            );

            // Search bar
            let search_line = if search.is_empty() {
                Line::from(Span::styled(
                    "  / search…",
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(vec![
                    Span::styled("  / ", Style::default().fg(Color::Yellow)),
                    Span::styled(search.as_str(), Style::default().fg(Color::White)),
                    Span::styled("█", Style::default().fg(Color::Cyan)),
                ])
            };
            f.render_widget(Paragraph::new(search_line), chunks[2]);
        }

        State::Confirm { events } => {
            let event_labels: Vec<String> = events.iter().map(|e| e.label().to_string()).collect();
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Review",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!("  Events   : {}", event_labels.join(", "))),
                    Line::from(
                        "  Extension: ~/.pi/agent/extensions/pi-notification.ts".to_string(),
                    ),
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

        State::Done { events } => {
            let event_labels: Vec<String> = events.iter().map(|e| e.label().to_string()).collect();
            let lines = vec![
                Line::from(Span::styled(
                    "Installed!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("  Events : {}", event_labels.join(", "))),
                Line::from(""),
                Line::from(Span::styled(
                    "  Reload pi with /reload or restart to activate",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to exit",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
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

    fn events_state() -> State {
        State::SelectEvents {
            events: EventOpt::all_events(),
            cursor: 0,
            search: String::new(),
            active_tab: 0,
        }
    }

    #[test]
    fn test_welcome_enter_goes_to_select_events() {
        assert!(matches!(
            next_state(State::Welcome, KeyCode::Enter),
            State::SelectEvents { .. }
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
    fn test_select_events_space_toggles() {
        let state = events_state();
        let next = next_state(state, KeyCode::Char(' '));
        if let State::SelectEvents { events, .. } = next {
            // AgentEnd starts selected
            assert!(!events[0].selected);
        } else {
            panic!("expected SelectEvents");
        }
    }

    #[test]
    fn test_defaults_agent_end_checked() {
        let events = EventOpt::all_events();
        let get = |ev: &PiEvent| events.iter().find(|e| &e.event == ev).unwrap().selected;
        assert!(get(&PiEvent::AgentEnd));
        assert!(!get(&PiEvent::SessionStart));
        assert!(!get(&PiEvent::SessionShutdown));
        assert!(!get(&PiEvent::TurnEnd));
        assert!(!get(&PiEvent::ToolExecutionEnd));
    }

    #[test]
    fn test_all_events_present() {
        let events = EventOpt::all_events();
        assert_eq!(events.len(), 5);
    }
}
