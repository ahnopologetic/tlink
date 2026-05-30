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
        search: String,
        active_tab: usize, // 0 = All, 1..=N = category index
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
        use HookEvent::*;
        // (event, default_selected)
        let defs: &[(HookEvent, bool)] = &[
            // Notifications
            (NotificationIdle, true),
            (NotificationPermission, true),
            (NotificationAuth, false),
            (NotificationElicitDialog, false),
            (NotificationElicitComplete, false),
            (NotificationElicitResponse, false),
            (AllNotifications, false),
            // Turn
            (Stop, true),
            (StopFailure, false),
            // Tools
            (PostToolUse, false),
            (PostToolUseFailure, false),
            // Agents & Tasks
            (SubagentStop, false),
            (TeammateIdle, false),
            (TaskCreated, false),
            (TaskCompleted, false),
            // Session
            (SessionStart, false),
            (SessionEnd, false),
        ];
        defs.iter()
            .map(|(e, s)| Self {
                event: e.clone(),
                selected: *s,
            })
            .collect()
    }
}

pub const CATEGORIES: &[&str] = &[
    "Notifications",
    "Turn",
    "Tools",
    "Agents & Tasks",
    "Session",
];

/// Returns the indices into `events` that are visible given the current tab and search query.
pub fn visible_indices(events: &[EventOpt], search: &str, active_tab: usize) -> Vec<usize> {
    let q = search.to_lowercase();
    events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let tab_match =
                active_tab == 0 || CATEGORIES.get(active_tab - 1) == Some(&e.event.category());
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
        ) => State::SelectEvents {
            events: EventOpt::all_events(),
            cursor: 0,
            search: String::new(),
            active_tab: 0,
        },
        (State::SelectMethod { .. }, KeyCode::Char('q') | KeyCode::Esc) => State::Cancelled,

        // SelectEvents — Tab cycles categories, typing filters, Space toggles, Enter confirms
        (
            State::SelectEvents {
                events,
                cursor: _,
                search: _,
                active_tab,
            },
            KeyCode::Tab,
        ) => {
            let n_tabs = CATEGORIES.len() + 1; // 0 = All, 1..=N = category
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
                // Enter while searching: commit first visible match's toggle, clear search
                return State::SelectEvents {
                    events,
                    cursor: 0,
                    search: String::new(),
                    active_tab,
                };
            }
            let selected: Vec<HookEvent> = events
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.event.clone())
                .collect();
            let events_final = if selected.is_empty() {
                vec![HookEvent::NotificationIdle]
            } else {
                selected
            };
            State::Confirm {
                method: NotifMethod::Osascript,
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

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<Option<InstallOptions>> {
    let mut state = State::Welcome;
    let mut pending_method: Option<NotifMethod> = None;

    loop {
        terminal.draw(|f| render(f, &state))?;

        match state.clone() {
            State::Installing { method, events } => {
                let opts = InstallOptions {
                    method: method.clone(),
                    events: events.clone(),
                };
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
                if let State::Done {
                    ref method,
                    ref events,
                } = state
                {
                    return Ok(Some(InstallOptions {
                        method: method.clone(),
                        events: events.clone(),
                    }));
                }

                // Capture method before leaving SelectMethod
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
                    Line::from(
                        "a desktop notification showing the tmux location (session:window.pane).",
                    ),
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
                .highlight_style(Style::default()); // styling handled per-item above
            let mut ls = ListState::default();
            ls.select(Some(*cursor));
            f.render_stateful_widget(list, content, &mut ls);
        }

        State::SelectEvents {
            events,
            cursor,
            search,
            active_tab,
        } => {
            // Split content into tabs + list + search bar
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // tab bar
                    Constraint::Fill(1),   // event list
                    Constraint::Length(1), // search bar
                ])
                .split(content);

            // ── Tab bar ───────────────────────────────────────────────────
            let tab_spans: Vec<Span> = std::iter::once("All".to_string())
                .chain(CATEGORIES.iter().map(|c| c.to_string()))
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

            // ── Event list ────────────────────────────────────────────────
            let visible = visible_indices(events, search, *active_tab);
            let mut lines: Vec<Line> = Vec::new();
            let mut prev_cat = "";

            for &i in &visible {
                let e = &events[i];
                // Show category header only in "All" tab
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

            // ── Search bar ────────────────────────────────────────────────
            let search_line = if search.is_empty() {
                Line::from(Span::styled(
                    "  / search…",
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(vec![
                    Span::styled("  / ", Style::default().fg(Color::Yellow)),
                    Span::styled(search.as_str(), Style::default().fg(Color::White)),
                    Span::styled("█", Style::default().fg(Color::Cyan)), // cursor
                ])
            };
            f.render_widget(Paragraph::new(search_line), chunks[2]);
        }

        State::Confirm { method, events } => {
            let event_labels: Vec<String> = events.iter().map(|e| e.label().to_string()).collect();
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Review",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!("  Notification method : {}", method.label())),
                    Line::from(format!(
                        "  Hook events        : {}",
                        event_labels.join(", ")
                    )),
                    Line::from(
                        "  Hook script        : ~/.config/tlink/hooks/claude-notification.sh"
                            .to_string(),
                    ),
                    Line::from("  Claude settings    : ~/.claude/settings.json".to_string()),
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

        State::Done { method, events } => {
            let event_labels: Vec<String> = events.iter().map(|e| e.label().to_string()).collect();
            let mut lines = vec![
                Line::from(Span::styled(
                    "Installed!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("  Method : {}", method.label())),
                Line::from(format!("  Events : {}", event_labels.join(", "))),
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
            search: String::new(),
            active_tab: 0,
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
        assert!(matches!(
            next_state(State::Welcome, KeyCode::Esc),
            State::Cancelled
        ));
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
        assert!(matches!(
            next_state(state, KeyCode::Down),
            State::SelectMethod { cursor: 0, .. }
        ));
    }

    #[test]
    fn test_select_method_enter_goes_to_select_events() {
        assert!(matches!(
            next_state(method_state(), KeyCode::Enter),
            State::SelectEvents { .. }
        ));
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
        let state = events_state(); // NotificationIdle + NotificationPermission + Stop are on by default
        let next = next_state(state, KeyCode::Enter);
        if let State::Confirm { events, .. } = next {
            assert!(events.contains(&HookEvent::NotificationIdle));
        } else {
            panic!("expected Confirm");
        }
    }

    #[test]
    fn test_defaults_idle_permission_stop_checked() {
        let events = EventOpt::all_events();
        let get = |ev: &HookEvent| events.iter().find(|e| &e.event == ev).unwrap().selected;
        assert!(get(&HookEvent::NotificationIdle));
        assert!(get(&HookEvent::NotificationPermission));
        assert!(get(&HookEvent::Stop));
        assert!(!get(&HookEvent::AllNotifications));
        assert!(!get(&HookEvent::StopFailure));
    }

    #[test]
    fn test_select_events_enter_defaults_to_idle_when_none_selected() {
        let state = State::SelectEvents {
            events: EventOpt::all_events()
                .into_iter()
                .map(|mut e| {
                    e.selected = false;
                    e
                })
                .collect(),
            cursor: 0,
            search: String::new(),
            active_tab: 0,
        };
        if let State::Confirm { events, .. } = next_state(state, KeyCode::Enter) {
            assert_eq!(events, vec![HookEvent::NotificationIdle]);
        } else {
            panic!("expected Confirm");
        }
    }

    #[test]
    fn test_all_17_events_present() {
        use HookEvent::*;
        let events = EventOpt::all_events();
        let types: Vec<&HookEvent> = events.iter().map(|e| &e.event).collect();
        for expected in &[
            NotificationIdle,
            NotificationPermission,
            NotificationAuth,
            NotificationElicitDialog,
            NotificationElicitComplete,
            NotificationElicitResponse,
            AllNotifications,
            Stop,
            StopFailure,
            PostToolUse,
            PostToolUseFailure,
            SubagentStop,
            TeammateIdle,
            TaskCreated,
            TaskCompleted,
            SessionStart,
            SessionEnd,
        ] {
            assert!(types.contains(&expected), "missing {:?}", expected);
        }
        assert_eq!(events.len(), 17);
    }

    #[test]
    fn test_confirm_enter_goes_to_installing() {
        let state = State::Confirm {
            method: NotifMethod::Osascript,
            events: vec![HookEvent::NotificationIdle],
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
            events: vec![HookEvent::NotificationIdle],
        };
        assert!(matches!(
            next_state(state, KeyCode::Char('n')),
            State::Cancelled
        ));
    }
}
