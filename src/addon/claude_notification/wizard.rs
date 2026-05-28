use super::{HookEvent, InstallOptions, NotifMethod};
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
    SelectEvents {
        events: Vec<EventOpt>,
        cursor: usize,
    },
    Confirm {
        method: NotifMethod,
        events: Vec<HookEvent>,
    },
    Installing {
        method: NotifMethod,
        events: Vec<HookEvent>,
    },
    Done {
        method: NotifMethod,
        events: Vec<HookEvent>,
    },
    Cancelled,
}

#[derive(Clone)]
pub struct EventOpt {
    pub event: HookEvent,
    pub selected: bool,
}

impl EventOpt {
    fn all_events() -> Vec<Self> {
        vec![
            // Pre-checked: the two events users almost always want
            Self { event: HookEvent::IdlePrompt,          selected: true  },
            Self { event: HookEvent::PermissionPrompt,    selected: true  },
            // Opt-in: lower signal or MCP-specific
            Self { event: HookEvent::AuthSuccess,         selected: false },
            Self { event: HookEvent::ElicitationDialog,   selected: false },
            Self { event: HookEvent::ElicitationComplete, selected: false },
            Self { event: HookEvent::ElicitationResponse, selected: false },
            // Catch-all: overrides individual selections
            Self { event: HookEvent::All,                 selected: false },
        ]
    }
}

pub fn next_state(state: State, key: KeyCode) -> State {
    match (state, key) {
        // Welcome
        (State::Welcome, KeyCode::Enter) => {
            let methods = NotifMethod::platform_methods();
            State::SelectMethod { methods, cursor: 0 }
        }
        (State::Welcome, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // SelectMethod
        (State::SelectMethod { methods, cursor }, KeyCode::Up) => State::SelectMethod {
            methods,
            cursor: cursor.saturating_sub(1),
        },
        (State::SelectMethod { methods, cursor }, KeyCode::Down) => {
            let max = methods.len().saturating_sub(1);
            State::SelectMethod { methods, cursor: (cursor + 1).min(max) }
        }
        (State::SelectMethod { methods: _, cursor: _ }, KeyCode::Enter) => State::SelectEvents {
            events: EventOpt::all_events(),
            cursor: 0,
        },
        (State::SelectMethod { .. }, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // SelectEvents — Space toggles, Enter confirms
        (State::SelectEvents { events, cursor }, KeyCode::Up) => State::SelectEvents {
            cursor: cursor.saturating_sub(1),
            events,
        },
        (State::SelectEvents { events, cursor }, KeyCode::Down) => {
            let max = events.len().saturating_sub(1);
            State::SelectEvents { cursor: (cursor + 1).min(max), events }
        }
        (State::SelectEvents { mut events, cursor }, KeyCode::Char(' ')) => {
            events[cursor].selected = !events[cursor].selected;
            State::SelectEvents { events, cursor }
        }
        (State::SelectEvents { events, .. }, KeyCode::Enter) => {
            let selected: Vec<HookEvent> = events
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.event.clone())
                .collect();
            // Default to idle_prompt if nothing selected
            let events_final = if selected.is_empty() {
                vec![HookEvent::IdlePrompt]
            } else {
                selected
            };
            // We need the method from SelectMethod — but state machine lost it.
            // Handled in event_loop which keeps pending_method.
            State::Confirm {
                method: NotifMethod::Osascript, // placeholder, replaced in event_loop
                events: events_final,
            }
        }
        (State::SelectEvents { .. }, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // Confirm
        (State::Confirm { method, events }, KeyCode::Enter | KeyCode::Char('y')) => {
            State::Installing { method, events }
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

// ── Event loop (handles Installing auto-transition and keeps pending state) ───

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Option<InstallOptions>> {
    let mut state = State::Welcome;
    let mut pending_method: Option<NotifMethod> = None;

    loop {
        terminal.draw(|f| render(f, &state))?;

        match state.clone() {
            State::Installing { method, events } => {
                let opts = InstallOptions { method: method.clone(), events: events.clone() };
                super::install_with_options(&opts)?;
                state = State::Done { method, events };
                continue;
            }
            State::Cancelled => return Ok(None),
            // Done is rendered once then exits on any key (handled in next_state / below)
            _ => {}
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                // Done: exit BEFORE calling next_state (which would transition to Cancelled)
                if let State::Done { ref method, ref events } = state {
                    return Ok(Some(InstallOptions {
                        method: method.clone(),
                        events: events.clone(),
                    }));
                }

                // Capture method before leaving SelectMethod
                if let State::SelectMethod { ref methods, cursor } = state {
                    if code == KeyCode::Enter {
                        pending_method = Some(methods[cursor].clone());
                    }
                }

                state = next_state(state, code);

                // Inject the captured method into Confirm
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
        .title(" tlink · install claude-notification ")
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
                        "claude-notification add-on",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from("When Claude finishes a task or needs your attention, you'll get"),
                    Line::from("a desktop notification showing the tmux location (session:window.pane)."),
                    Line::from(""),
                    Line::from("This wizard will configure:"),
                    Line::from("  • Notification delivery method"),
                    Line::from("  • Which Claude events trigger a notification"),
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
            let items: Vec<ListItem> = methods
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let selected = i == *cursor;
                    let prefix = if selected { "❯ " } else { "  " };
                    let name_style = if selected {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let tag = if m.available() {
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
                        Line::from(Span::styled(
                            format!("    {}", m.description()),
                            desc_style,
                        )),
                        Line::from(""),
                    ])
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().title(
                    "Notification method  (↑/↓ move  •  Enter select  •  q quit)",
                ))
                .highlight_style(Style::default()); // styling handled per-item above
            let mut ls = ListState::default();
            ls.select(Some(*cursor));
            f.render_stateful_widget(list, content, &mut ls);
        }

        State::SelectEvents { events, cursor } => {
            let items: Vec<ListItem> = events
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let selected_row = i == *cursor;
                    let prefix = if selected_row { "❯ " } else { "  " };
                    let check = if e.selected {
                        Span::styled("[x] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
                    };
                    let label_style = if selected_row {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let desc_style = if selected_row {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::raw(prefix),
                            check,
                            Span::styled(e.event.label(), label_style),
                        ]),
                        Line::from(Span::styled(
                            format!("       {}", e.event.description()),
                            desc_style,
                        )),
                        Line::from(""),
                    ])
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().title(
                    "Hook events  (↑/↓ move  •  Space toggle  •  Enter confirm  •  q quit)",
                ))
                .highlight_style(Style::default()); // styling handled per-item above
            let mut ls = ListState::default();
            ls.select(Some(*cursor));
            f.render_stateful_widget(list, content, &mut ls);
        }

        State::Confirm { method, events } => {
            let event_labels: Vec<String> = events.iter().map(|e| e.label().to_string()).collect();
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled("Review", Style::default().add_modifier(Modifier::BOLD))),
                    Line::from(""),
                    Line::from(format!("  Notification method : {}", method.label())),
                    Line::from(format!("  Hook events        : {}", event_labels.join(", "))),
                    Line::from(format!(
                        "  Hook script        : ~/.config/tlink/hooks/claude-notification.sh"
                    )),
                    Line::from(format!(
                        "  Claude settings    : ~/.claude/settings.json"
                    )),
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
            f.render_widget(
                Paragraph::new("Installing…"),
                content,
            );
        }

        State::Done { method, events } => {
            let event_labels: Vec<String> = events.iter().map(|e| e.label().to_string()).collect();
            let mut lines = vec![
                Line::from(Span::styled(
                    "Installed!",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("  Method : {}", method.label())),
                Line::from(format!("  Events : {}", event_labels.join(", "))),
                Line::from(""),
            ];
            if *method == NotifMethod::Osascript || *method == NotifMethod::NotifySend {
                lines.push(Line::from(Span::styled(
                    "  Tip: install terminal-notifier (macOS) or dunst (Linux) for click-to-navigate",
                    Style::default().fg(Color::Yellow),
                )));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                "  Note: this runs alongside Claude Code's built-in preferredNotifChannel setting",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
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

    fn events_state() -> State {
        State::SelectEvents {
            events: EventOpt::all_events(),
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
        assert!(matches!(next_state(State::Welcome, KeyCode::Char('q')), State::Cancelled));
        assert!(matches!(next_state(State::Welcome, KeyCode::Esc), State::Cancelled));
    }

    #[test]
    fn test_select_method_down_moves_cursor() {
        let next = next_state(method_state(), KeyCode::Down);
        assert!(matches!(next, State::SelectMethod { cursor: 1, .. }));
    }

    #[test]
    fn test_select_method_down_clamps() {
        let state = State::SelectMethod {
            methods: vec![NotifMethod::Osascript],
            cursor: 0,
        };
        assert!(matches!(next_state(state, KeyCode::Down), State::SelectMethod { cursor: 0, .. }));
    }

    #[test]
    fn test_select_method_enter_goes_to_select_events() {
        assert!(matches!(next_state(method_state(), KeyCode::Enter), State::SelectEvents { .. }));
    }

    #[test]
    fn test_select_events_space_toggles() {
        let state = events_state(); // idle_prompt starts selected=true
        let next = next_state(state, KeyCode::Char(' '));
        if let State::SelectEvents { events, .. } = next {
            assert!(!events[0].selected); // toggled off
        } else {
            panic!("expected SelectEvents");
        }
    }

    #[test]
    fn test_select_events_enter_collects_selected() {
        let state = events_state(); // idle_prompt is selected by default
        let next = next_state(state, KeyCode::Enter);
        if let State::Confirm { events, .. } = next {
            assert!(events.contains(&HookEvent::IdlePrompt));
        } else {
            panic!("expected Confirm");
        }
    }

    #[test]
    fn test_select_events_defaults_idle_and_permission_prechecked() {
        let events = EventOpt::all_events();
        assert!(events.iter().find(|e| e.event == HookEvent::IdlePrompt).unwrap().selected);
        assert!(events.iter().find(|e| e.event == HookEvent::PermissionPrompt).unwrap().selected);
        assert!(!events.iter().find(|e| e.event == HookEvent::All).unwrap().selected);
    }

    #[test]
    fn test_select_events_enter_defaults_to_idle_when_none_selected() {
        let state = State::SelectEvents {
            events: EventOpt::all_events()
                .into_iter()
                .map(|mut e| { e.selected = false; e })
                .collect(),
            cursor: 0,
        };
        if let State::Confirm { events, .. } = next_state(state, KeyCode::Enter) {
            assert_eq!(events, vec![HookEvent::IdlePrompt]);
        } else {
            panic!("expected Confirm");
        }
    }

    #[test]
    fn test_all_six_notification_types_present() {
        let events = EventOpt::all_events();
        let types: Vec<&HookEvent> = events.iter().map(|e| &e.event).collect();
        assert!(types.contains(&&HookEvent::IdlePrompt));
        assert!(types.contains(&&HookEvent::PermissionPrompt));
        assert!(types.contains(&&HookEvent::AuthSuccess));
        assert!(types.contains(&&HookEvent::ElicitationDialog));
        assert!(types.contains(&&HookEvent::ElicitationComplete));
        assert!(types.contains(&&HookEvent::ElicitationResponse));
    }

    #[test]
    fn test_confirm_enter_goes_to_installing() {
        let state = State::Confirm {
            method: NotifMethod::Osascript,
            events: vec![HookEvent::IdlePrompt],
        };
        assert!(matches!(next_state(state, KeyCode::Enter), State::Installing { .. }));
    }

    #[test]
    fn test_confirm_n_cancels() {
        let state = State::Confirm {
            method: NotifMethod::Osascript,
            events: vec![HookEvent::IdlePrompt],
        };
        assert!(matches!(next_state(state, KeyCode::Char('n')), State::Cancelled));
    }
}
