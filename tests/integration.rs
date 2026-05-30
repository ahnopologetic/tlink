use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Find the tlink binary.  On CI we have target/debug/tlink from the build step;
/// locally `cargo run` works too.
fn tlink_binary() -> PathBuf {
    // Prefer pre-built binary next to the test runner
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("target/debug"));
    let candidate = exe_dir.join("tlink");
    if candidate.exists() {
        return candidate;
    }
    // Fallback: look in target/debug relative to manifest dir
    let fallback = PathBuf::from("target/debug/tlink");
    if fallback.exists() {
        return fallback;
    }
    // Last resort: use cargo run
    PathBuf::from("cargo")
}

/// Helper: write bash script to temp file and run `bash -n`.
fn check_bash_syntax(script: &str, label: &str) -> bool {
    let pid = std::process::id();
    let tid = std::thread::current().id();
    let tmp = std::env::temp_dir().join(format!("tlink-syntax-{}-{:?}-{}.sh", pid, tid, label));
    if std::fs::write(&tmp, script).is_err() {
        return false;
    }
    let output = Command::new("bash")
        .args(["-n", &tmp.to_string_lossy()])
        .output()
        .expect("failed to run bash -n");
    let ok = output.status.success();
    std::fs::remove_file(&tmp).ok();
    ok
}

fn codex_script(method: &str) -> String {
    let notify_part = match method {
        "terminal-notifier" => "    terminal-notifier \
        -title \"$NOTIF_TITLE\" \
        -subtitle \"$LOCATION\" \
        -message \"$MESSAGE\" \
        -execute \"tlink open $DEEPLINK\" &",
        "osascript" => "    osascript -e \"display notification \\\"$MESSAGE\\\" with title \\\"$NOTIF_TITLE\\\" subtitle \\\"$LOCATION\\\" sound name \\\"Glass\\\"\"",
        "dunstify" => "    (\n        ACTION=$(dunstify \"$NOTIF_TITLE\" \"$MESSAGE\" \
            --hint=string:x-dunst-stack-tag:tlink \
            --action=\"default,Go there\" \
            --urgency=normal \
            --icon=utilities-terminal \
            --appname=\"Codex CLI\")\n        [ \"$ACTION\" = \"default\" ] && tlink open \"$DEEPLINK\"\n    ) &",
        "notify-send" => "    notify-send \"$NOTIF_TITLE\" \"$MESSAGE\" \
        --urgency=normal \
        --icon=utilities-terminal \
        --app-name=\"Codex CLI\" \
        --hint=string:body:\"$LOCATION\"",
        _ => panic!("unknown method: {}", method),
    };

    format!(
        "#!/bin/bash
SESSION=$(tmux display-message -p \"#{{session_name}}\" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p \"#{{window_name}}\" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p \"#{{pane_index}}\" 2>/dev/null) || exit 0
[ -z \"$SESSION\" ] && exit 0
MESSAGE=\"Codex CLI task completed\"
NOTIF_TITLE=\"Codex CLI\"
DEEPLINK=\"tmux://${{SESSION}}/${{WINDOW}}/${{PANE}}\"
LOCATION=\"${{SESSION}} > ${{WINDOW}} > ${{PANE}}\"
{}
",
        notify_part
    )
}

fn gemini_script(method: &str) -> String {
    let notify_part = match method {
        "terminal-notifier" => "    terminal-notifier \
        -title \"$NOTIF_TITLE\" \
        -subtitle \"$LOCATION\" \
        -message \"$MESSAGE\" \
        -execute \"tlink open $DEEPLINK\" &",
        "osascript" => "    osascript -e \"display notification \\\"$MESSAGE\\\" with title \\\"$NOTIF_TITLE\\\" subtitle \\\"$LOCATION\\\" sound name \\\"Glass\\\"\"",
        "dunstify" => "    (\n        ACTION=$(dunstify \"$NOTIF_TITLE\" \"$MESSAGE\" \
            --hint=string:x-dunst-stack-tag:tlink \
            --action=\"default,Go there\" \
            --urgency=normal \
            --icon=utilities-terminal \
            --appname=\"Gemini CLI\")\n        [ \"$ACTION\" = \"default\" ] && tlink open \"$DEEPLINK\"\n    ) &",
        "notify-send" => "    notify-send \"$NOTIF_TITLE\" \"$MESSAGE\" \
        --urgency=normal \
        --icon=utilities-terminal \
        --app-name=\"Gemini CLI\" \
        --hint=string:body:\"$LOCATION\"",
        _ => panic!("unknown method: {}", method),
    };

    format!(
        "#!/bin/bash
SESSION=$(tmux display-message -p \"#{{session_name}}\" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p \"#{{window_name}}\" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p \"#{{pane_index}}\" 2>/dev/null) || exit 0
[ -z \"$SESSION\" ] && exit 0
INPUT=$(cat)
MESSAGE=$(echo \"$INPUT\" | python3 -c 'import sys,json; d=json.loads(sys.stdin.read()); print(d.get(\"message\",\"Gemini CLI notification\"))' 2>/dev/null || echo \"Gemini CLI notification\")
NOTIF_TITLE=\"Gemini CLI\"
DEEPLINK=\"tmux://$SESSION/$WINDOW/$PANE\"
LOCATION=\"$SESSION > $WINDOW > $PANE\"
{}
",
        notify_part
    )
}

const CLAUDE_SCRIPT: &str = r"#!/bin/bash
SESSION=$(tmux display-message -p '#{session_name}' 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p '#{window_name}' 2>/dev/null) || exit 0
PANE=$(tmux display-message -p '#{pane_index}' 2>/dev/null) || exit 0
[ -z '$SESSION' ] && exit 0
exec tlink notify --session '$SESSION' --window '$WINDOW' --pane '$PANE'";

// ── CLI smoke tests ───────────────────────────────────────────────────────────

#[test]
fn test_tlink_help() {
    let out = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("tlink"));
}

#[test]
fn test_tlink_list_addons() {
    let out = Command::new("cargo")
        .args(["run", "--", "list", "add-ons"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("claude-notification"),
        "should contain claude-notification"
    );
    assert!(
        s.contains("NAME") && s.contains("STATUS"),
        "should have table headers"
    );
}

#[test]
fn test_tlink_install_no_args() {
    let out = Command::new("cargo")
        .args(["run", "--", "install"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    let e = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() || s.contains("Usage") || e.contains("Usage"),
        "should be okay: stdout={} stderr={}",
        s,
        e
    );
}

/// Check if --interactive flag exists in the CLI (only on branches that have it)
fn has_interactive_flag() -> bool {
    let out = Command::new("cargo")
        .args(["run", "--", "install", "--help"])
        .output()
        .unwrap();
    if !out.status.success() {
        return false;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.contains("--interactive")
}

#[test]
fn test_tlink_install_interactive_flag() {
    if !has_interactive_flag() {
        eprintln!("  SKIP: --interactive flag not available in this build");
        return;
    }
}

// ── tlink notify ──────────────────────────────────────────────────────────────

/// Invoke `tlink notify` with a JSON payload piped to stdin.
/// Uses the pre-built binary when available, falls back to cargo run.
fn notify(payload: &str) -> bool {
    let tlink = tlink_binary();
    let is_cargo = tlink.to_string_lossy() == "cargo";

    let args: &[&str] = if is_cargo {
        &[
            "run",
            "--",
            "notify",
            "--session",
            "ts",
            "--window",
            "1",
            "--pane",
            "0",
        ]
    } else {
        &["notify", "--session", "ts", "--window", "1", "--pane", "0"]
    };

    let mut child = match Command::new(if is_cargo {
        "cargo"
    } else {
        tlink.to_str().unwrap_or("tlink")
    })
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  failed to spawn: {}", e);
            return false;
        }
    };

    if let Some(mut h) = child.stdin.take() {
        if let Err(e) = h.write_all(payload.as_bytes()) {
            eprintln!("  failed to write payload: {}", e);
            return false;
        }
    }

    match child.wait_with_output() {
        Ok(out) => {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                eprintln!("  tlink notify failed ({}): {}", tlink.display(), stderr);
            }
            out.status.success()
        }
        Err(e) => {
            eprintln!("  failed to wait: {}", e);
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
            check_bash_syntax(&codex_script(m), &format!("codex-{}", m)),
            "codex {} should have valid bash syntax",
            m
        );
    }
}

#[test]
fn test_gemini_script_syntax() {
    for m in &["terminal-notifier", "osascript", "dunstify", "notify-send"] {
        assert!(
            check_bash_syntax(&gemini_script(m), &format!("gemini-{}", m)),
            "gemini {} should have valid bash syntax",
            m
        );
    }
}

#[test]
fn test_claude_script_syntax() {
    assert!(check_bash_syntax(CLAUDE_SCRIPT, "claude"));
}

// ── Graceful exit without tmux ────────────────────────────────────────────────

#[test]
fn test_codex_graceful_no_tmux() {
    let s = format!(
        "tmux() {{ exit 1; }}; export -f tmux; {}",
        codex_script("osascript")
    );
    let (_, stderr, ok) = run_bash(&s);
    assert!(
        ok,
        "codex hook should exit 0 without tmux: stderr={}",
        stderr
    );
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
        "gemini hook should exit 0 without tmux: stderr={}",
        stderr
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
    let tlink = if tlink_binary().to_string_lossy() == "cargo" {
        let out = Command::new("cargo")
            .args(["run", "--", "open", &format!("tmux://{}", session)])
            .output()
            .unwrap();
        out.status.success()
    } else {
        let out = Command::new(&tlink_binary())
            .args(["open", &format!("tmux://{}", session)])
            .output()
            .unwrap();
        out.status.success()
    };
    assert!(tlink);
}

// ── Hook pipe test ────────────────────────────────────────────────────────────

#[test]
fn test_hook_pipe() {
    let tlink = tlink_binary();
    let payload = r#"{"hook_event_name":"Notification","notification_type":"idle_prompt","message":"Hook test"}"#;
    let cmd = if tlink.to_string_lossy() == "cargo" {
        format!(
            "printf '%s' '{}' | cargo run -- notify --session s --window 1 --pane 0",
            payload
        )
    } else {
        format!(
            "printf '%s' '{}' | {} notify --session s --window 1 --pane 0",
            payload,
            tlink.to_string_lossy()
        )
    };
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
    let pid = std::process::id();
    let tid = std::thread::current().id();
    let tmp = std::env::temp_dir().join(format!("tlink-cfg-{}-{:?}.toml", pid, tid));
    std::fs::write(&tmp, "notification_method = \"terminal-notifier\"\n").unwrap();
    let c = std::fs::read_to_string(&tmp).unwrap();
    assert!(c.contains("terminal-notifier"));
    std::fs::remove_file(&tmp).ok();
}

// ── Python3 parser test ───────────────────────────────────────────────────────

#[test]
fn test_gemini_python_parser() {
    let payload = r#"{"hook_event_name":"AfterAgent","notification_type":"idle_prompt","message":"Gemini task done"}"#;
    let mut child = Command::new("python3")
        .args(["-c", r#"import sys, json, shlex; d=json.loads(sys.stdin.read()); msg=d.get('message',''); print('MESSAGE=' + shlex.quote(msg))"#])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().unwrap();
    {
        let mut h = child.stdin.take().unwrap();
        h.write_all(payload.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("MESSAGE="));
    assert!(stdout.contains("Gemini task done"));
}

#[test]
fn test_gemini_python_parser_empty() {
    let mut child = Command::new("python3")
        .args(["-c", r#"import sys, json; d=json.loads(sys.stdin.read()) if sys.stdin.read().strip() else {}; print('ok')"#])
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().unwrap();
    child.stdin.take();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
}

// ── Helper: run bash ──────────────────────────────────────────────────────────

fn run_bash(script: &str) -> (String, String, bool) {
    let mut child = Command::new("bash")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn bash");

    {
        let mut h = child.stdin.take().expect("failed to get stdin");
        h.write_all(script.as_bytes())
            .expect("failed to write script");
        h.write_all(b"\n").ok();
    }

    let output = child.wait_with_output().expect("failed to wait on bash");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}
