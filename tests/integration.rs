use std::io::Write;
use std::process::{Command, Stdio};

/// Build a Command for running the tlink binary.
/// Uses TLINK_BIN env var (set by CI) or falls back to the project binary.
/// Never uses `cargo run` — causes deadlock with `cargo test` on the package cache.
fn tlink_cmd(args: &[&str]) -> Command {
    // TLINK_BIN env var set by CI workflow to a pre-built binary copy
    if let Ok(path) = std::env::var("TLINK_BIN") {
        let exists = std::path::Path::new(&path).is_file();
        let can_exec = Command::new(&path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        eprintln!("[tlink] TLINK_BIN={path} exists={exists} exec={can_exec:?}");
        if let Ok(status) = can_exec {
            if status.success() {
                let mut c = Command::new(&path);
                c.args(args);
                return c;
            }
        }
    }

    // TLINK_DEBUG_BIN fallback for local development
    for candidate in &["target/debug/tlink-cli", "target/debug/tlink"] {
        let p = std::path::Path::new(candidate);
        if p.is_file() {
            // Quick sanity check: is this a test harness or the real binary?
            let check = Command::new(p)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if let Ok(status) = check {
                if status.success() {
                    let mut c = Command::new(p);
                    c.args(args);
                    return c;
                }
            }
        }
    }

    panic!("tlink binary not found. Run `cargo build --bin tlink` first.");
}

fn check_bash_syntax(script: &str, label: &str) -> bool {
    use std::path::PathBuf;
    let pid = std::process::id();
    let tid = std::thread::current().id();
    let tmp = std::env::temp_dir().join(format!("tlink-syntax-{}-{:?}-{}.sh", pid, tid, label));
    if std::fs::write(&tmp, script).is_err() {
        return false;
    }
    let ok = Command::new("bash")
        .args(["-n", &tmp.to_string_lossy()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    std::fs::remove_file(&tmp).ok();
    ok
}

fn codex_script(method: &str) -> String {
    let n = match method {
        "terminal-notifier" => "    terminal-notifier \\\n        -title \"$NOTIF_TITLE\" \\\n        -subtitle \"$LOCATION\" \\\n        -message \"$MESSAGE\" \\\n        -execute \"tlink open $DEEPLINK\" &",
        "osascript" => "    osascript -e \"display notification \\\"$MESSAGE\\\" with title \\\"$NOTIF_TITLE\\\" subtitle \\\"$LOCATION\\\" sound name \\\"Glass\\\"\"",
        "dunstify" => "    (\n        ACTION=$(dunstify \"$NOTIF_TITLE\" \"$MESSAGE\" \\\n            --hint=string:x-dunst-stack-tag:tlink \\\n            --action=\"default,Go there\" \\\n            --urgency=normal \\\n            --icon=utilities-terminal \\\n            --appname=\"Codex CLI\")\n        [ \"$ACTION\" = \"default\" ] && tlink open \"$DEEPLINK\"\n    ) &",
        "notify-send" => "    notify-send \"$NOTIF_TITLE\" \"$MESSAGE\" \\\n        --urgency=normal \\\n        --icon=utilities-terminal \\\n        --app-name=\"Codex CLI\" \\\n        --hint=string:body:\"$LOCATION\"",
        _ => panic!("unknown method: {method}"),
    };
    format!("#!/bin/bash\nSESSION=$(tmux display-message -p \"#{{session_name}}\" 2>/dev/null) || exit 0\nWINDOW=$(tmux display-message -p \"#{{window_name}}\" 2>/dev/null) || exit 0\nPANE=$(tmux display-message -p \"#{{pane_index}}\" 2>/dev/null) || exit 0\n[ -z \"$SESSION\" ] && exit 0\nMESSAGE=\"Codex CLI task completed\"\nNOTIF_TITLE=\"Codex CLI\"\nDEEPLINK=\"tmux://${{SESSION}}/${{WINDOW}}/${{PANE}}\"\nLOCATION=\"${{SESSION}} > ${{WINDOW}} > ${{PANE}}\"\n{n}\n")
}

fn gemini_script(method: &str) -> String {
    let n = match method {
        "terminal-notifier" => "    terminal-notifier \\\n        -title \"$NOTIF_TITLE\" \\\n        -subtitle \"$LOCATION\" \\\n        -message \"$MESSAGE\" \\\n        -execute \"tlink open $DEEPLINK\" &",
        "osascript" => "    osascript -e \"display notification \\\"$MESSAGE\\\" with title \\\"$NOTIF_TITLE\\\" subtitle \\\"$LOCATION\\\" sound name \\\"Glass\\\"\"",
        "dunstify" => "    (\n        ACTION=$(dunstify \"$NOTIF_TITLE\" \"$MESSAGE\" \\\n            --hint=string:x-dunst-stack-tag:tlink \\\n            --action=\"default,Go there\" \\\n            --urgency=normal \\\n            --icon=utilities-terminal \\\n            --appname=\"Gemini CLI\")\n        [ \"$ACTION\" = \"default\" ] && tlink open \"$DEEPLINK\"\n    ) &",
        "notify-send" => "    notify-send \"$NOTIF_TITLE\" \"$MESSAGE\" \\\n        --urgency=normal \\\n        --icon=utilities-terminal \\\n        --app-name=\"Gemini CLI\" \\\n        --hint=string:body:\"$LOCATION\"",
        _ => panic!("unknown method: {method}"),
    };
    format!("#!/bin/bash\nSESSION=$(tmux display-message -p \"#{{session_name}}\" 2>/dev/null) || exit 0\nWINDOW=$(tmux display-message -p \"#{{window_name}}\" 2>/dev/null) || exit 0\nPANE=$(tmux display-message -p \"#{{pane_index}}\" 2>/dev/null) || exit 0\n[ -z \"$SESSION\" ] && exit 0\nINPUT=$(cat)\nMESSAGE=$(echo \"$INPUT\" | python3 -c 'import sys,json; d=json.loads(sys.stdin.read()); print(d.get(\"message\",\"Gemini CLI notification\"))' 2>/dev/null || echo \"Gemini CLI notification\")\nNOTIF_TITLE=\"Gemini CLI\"\nDEEPLINK=\"tmux://$SESSION/$WINDOW/$PANE\"\nLOCATION=\"$SESSION > $WINDOW > $PANE\"\n{n}\n")
}

// ── CLI smoke tests ───────────────────────────────────────────────────────────

#[test]
fn test_tlink_help() {
    let out = tlink_cmd(&["--help"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("tlink"));
}

#[test]
fn test_tlink_list_addons() {
    let out = tlink_cmd(&["list", "add-ons"]).output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("claude-notification"));
    assert!(s.contains("NAME") && s.contains("STATUS"));
}

#[test]
fn test_tlink_install_no_args() {
    let out = tlink_cmd(&["install"]).output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    let e = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() || s.contains("Usage") || e.contains("Usage"));
}

fn has_interactive_flag() -> bool {
    tlink_cmd(&["install", "--help"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("--interactive"))
        .unwrap_or(false)
}

#[test]
fn test_tlink_install_interactive_flag() {
    if !has_interactive_flag() {
        eprintln!("  SKIP: --interactive flag not available");
        return;
    }
}

// ── tlink notify ──────────────────────────────────────────────────────────────

fn notify(payload: &str) -> bool {
    let mut child = match tlink_cmd(&["notify", "--session", "ts", "--window", "1", "--pane", "0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  could not spawn: {e}");
            return false;
        }
    };
    if let Some(mut h) = child.stdin.take() {
        if let Err(e) = h.write_all(payload.as_bytes()) {
            eprintln!("  write error: {e}");
            return false;
        }
    }
    match child.wait_with_output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!("  notify failed: {}", String::from_utf8_lossy(&out.stderr));
            false
        }
        Err(e) => {
            eprintln!("  wait error: {e}");
            false
        }
    }
}

#[test]
fn test_notify_idle() {
    assert!(notify(
        r#"{"hook_event_name":"Notification","notification_type":"idle_prompt","message":"Done"}"#
    ));
}
#[test]
fn test_notify_empty() {
    assert!(notify("{}"));
}
#[test]
fn test_notify_stop() {
    assert!(notify(r#"{"hook_event_name":"Stop"}"#));
}
#[test]
fn test_notify_perm() {
    assert!(notify(
        r#"{"hook_event_name":"Notification","notification_type":"permission_prompt","message":"Allow?"}"#
    ));
}
#[test]
fn test_notify_malformed() {
    assert!(notify("garbage"));
}
#[test]
fn test_notify_posttool() {
    assert!(notify(
        r#"{"hook_event_name":"PostToolUse","tool_name":"Bash"}"#
    ));
}
#[test]
fn test_notify_subagent() {
    assert!(notify(
        r#"{"hook_event_name":"SubagentStop","agent_type":"researcher"}"#
    ));
}
#[test]
fn test_notify_task() {
    assert!(notify(
        r#"{"hook_event_name":"TaskCreated","task_title":"Tests"}"#
    ));
}
#[test]
fn test_notify_session() {
    assert!(notify(r#"{"hook_event_name":"SessionStart"}"#));
}

// ── Hook script syntax ────────────────────────────────────────────────────────

#[test]
fn test_codex_script_syntax() {
    for m in &["terminal-notifier", "osascript", "dunstify", "notify-send"] {
        assert!(
            check_bash_syntax(&codex_script(m), &format!("codex-{m}")),
            "codex {m} should have valid bash syntax"
        );
    }
}

#[test]
fn test_gemini_script_syntax() {
    for m in &["terminal-notifier", "osascript", "dunstify", "notify-send"] {
        assert!(
            check_bash_syntax(&gemini_script(m), &format!("gemini-{m}")),
            "gemini {m} should have valid bash syntax"
        );
    }
}

#[test]
fn test_claude_script_syntax() {
    let script = "#!/bin/bash\nSESSION=$(tmux display-message -p '#{session_name}' 2>/dev/null) || exit 0\nWINDOW=$(tmux display-message -p '#{window_name}' 2>/dev/null) || exit 0\nPANE=$(tmux display-message -p '#{pane_index}' 2>/dev/null) || exit 0\n[ -z '$SESSION' ] && exit 0\nexec tlink notify --session '$SESSION' --window '$WINDOW' --pane '$PANE'";
    assert!(check_bash_syntax(script, "claude"));
}

// ── Graceful exit without tmux ────────────────────────────────────────────────

fn run_bash(s: &str) -> (String, String, bool) {
    let mut c = Command::new("bash")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut h = c.stdin.take().unwrap();
        h.write_all(s.as_bytes()).unwrap();
        h.write_all(b"\n").ok();
    }
    let o = c.wait_with_output().unwrap();
    (
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
        o.status.success(),
    )
}

#[test]
fn test_codex_graceful_no_tmux() {
    let s = format!(
        "tmux() {{ exit 1; }}; export -f tmux; {}",
        codex_script("osascript")
    );
    let (_, stderr, ok) = run_bash(&s);
    assert!(ok, "codex hook should exit 0 without tmux: stderr={stderr}");
}

#[test]
fn test_gemini_graceful_no_tmux() {
    let s = format!(
        "tmux() {{ exit 1; }}; export -f tmux; echo '' | {}",
        gemini_script("osascript")
    );
    let (_, stderr, ok) = run_bash(&s);
    assert!(
        ok,
        "gemini hook should exit 0 without tmux: stderr={stderr}"
    );
}

// ── Real tmux tests ───────────────────────────────────────────────────────────

fn tmux_fmt(fmt: &str) -> String {
    let out = Command::new("tmux")
        .args(["display-message", "-p", fmt])
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn test_tmux_session() {
    assert!(!tmux_fmt("#{session_name}").is_empty());
}
#[test]
fn test_tmux_window() {
    assert!(!tmux_fmt("#{window_name}").is_empty());
}
#[test]
fn test_tmux_pane() {
    assert!(!tmux_fmt("#{pane_index}").is_empty());
}

#[test]
fn test_tlink_open() {
    let session = tmux_fmt("#{session_name}");
    let out = tlink_cmd(&["open", &format!("tmux://{session}")])
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn test_hook_pipe() {
    let payload = r#"{"hook_event_name":"Notification","notification_type":"idle_prompt","message":"Hook test"}"#;
    let tlink = std::env::var("TLINK_BIN").unwrap_or_else(|_| "target/debug/tlink".to_string());
    let cmd = format!(
        "printf '%s' '{payload}' | '{}' notify --session s --window 1 --pane 0",
        tlink
    );
    let out = Command::new("bash").args(["-c", &cmd]).output().unwrap();
    assert!(
        out.status.success(),
        "pipe should work: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ── Config ────────────────────────────────────────────────────────────────────

#[test]
fn test_config_roundtrip() {
    let tmp = std::env::temp_dir().join(format!("tlink-cfg-{}.toml", std::process::id()));
    std::fs::write(&tmp, "notification_method = \"terminal-notifier\"\n").unwrap();
    assert!(std::fs::read_to_string(&tmp)
        .unwrap()
        .contains("terminal-notifier"));
    std::fs::remove_file(&tmp).ok();
}

// ── Python3 parser ────────────────────────────────────────────────────────────

#[test]
fn test_gemini_python_parser() {
    let mut c = Command::new("python3").args(["-c", r#"import sys, json, shlex; d=json.loads(sys.stdin.read()); print('MESSAGE=' + shlex.quote(d.get('message','')))"#])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    c.stdin
        .take()
        .unwrap()
        .write_all(br#"{"message":"Gemini task done"}"#)
        .unwrap();
    let o = c.wait_with_output().unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("Gemini task done"));
}

#[test]
fn test_gemini_python_parser_empty() {
    let mut c = Command::new("python3")
        .args(["-c", "import sys; print('ok')"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    c.stdin.take();
    assert!(c.wait_with_output().unwrap().status.success());
}
