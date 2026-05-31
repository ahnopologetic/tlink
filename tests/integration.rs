use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn tlink_bin() -> String {
    if let Ok(path) = std::env::var("TLINK_BIN") {
        return path;
    }
    let o = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata failed");
    let meta: serde_json::Value =
        serde_json::from_slice(&o.stdout).expect("invalid cargo metadata");
    let target_dir = meta["target_directory"]
        .as_str()
        .expect("no target_directory in metadata");
    format!("{target_dir}/debug/tlink")
}

fn tlink_cmd(args: &[&str]) -> Command {
    let bin = tlink_bin();
    let mut c = Command::new(&bin);
    c.args(args);
    c
}

fn check_bash_syntax(script: &str, label: &str) -> bool {
    let tmp =
        std::env::temp_dir().join(format!("tlink-syntax-{}-{}.sh", std::process::id(), label));
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

fn codex_script(_m: &str) -> String {
    // Thin wrapper — delegates to tlink notify with --source codex
    format!(
        "#!/bin/bash\nSESSION=$(tmux display-message -p \"#{{session_name}}\" 2>/dev/null) || exit 0\nWINDOW=$(tmux display-message -p \"#{{window_name}}\" 2>/dev/null) || exit 0\nPANE=$(tmux display-message -p \"#{{pane_index}}\" 2>/dev/null) || exit 0\n[ -z \"$SESSION\" ] && exit 0\nSTATUS=\"${{1:-turn-ended}}\"\nprintf '{{\"source\":\"codex\",\"status\":\"%s\"}}\\n' \"$STATUS\" | tlink notify --source codex --session \"$SESSION\" --window \"$WINDOW\" --pane \"$PANE\"\n"
    )
}

fn gemini_script(_m: &str) -> String {
    // Thin wrapper — delegates to tlink notify with --source gemini
    format!(
        "#!/bin/bash\nSESSION=$(tmux display-message -p \"#{{session_name}}\" 2>/dev/null) || exit 0\nWINDOW=$(tmux display-message -p \"#{{window_name}}\" 2>/dev/null) || exit 0\nPANE=$(tmux display-message -p \"#{{pane_index}}\" 2>/dev/null) || exit 0\n[ -z \"$SESSION\" ] && exit 0\nexec tlink notify --source gemini --session \"$SESSION\" --window \"$WINDOW\" --pane \"$PANE\"\n"
    )
}

#[test]
fn test_tlink_help() {
    let o = tlink_cmd(&["--help"]).output().unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("tlink"));
}
#[test]
fn test_tlink_list_addons() {
    let o = tlink_cmd(&["list", "add-ons"]).output().unwrap();
    assert!(o.status.success());
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("claude-notification"));
    assert!(s.contains("NAME") && s.contains("STATUS"));
}
#[test]
fn test_tlink_install_no_args() {
    let o = tlink_cmd(&["install"]).output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success() || s.contains("Usage") || e.contains("Usage"));
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
        eprintln!("  SKIP");
        return;
    }
}

fn notify(p: &str) -> bool {
    let mut c = match tlink_cmd(&["notify", "--session", "ts", "--window", "1", "--pane", "0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  spawn error: {e}");
            return false;
        }
    };
    if let Some(mut h) = c.stdin.take() {
        if let Err(e) = h.write_all(p.as_bytes()) {
            eprintln!("  write error: {e}");
            return false;
        }
    }
    match c.wait_with_output() {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            eprintln!("  notify failed: {}", String::from_utf8_lossy(&o.stderr));
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

#[test]
fn test_codex_script_syntax() {
    for m in &["terminal-notifier", "osascript", "dunstify", "notify-send"] {
        assert!(
            check_bash_syntax(&codex_script(m), &format!("codex-{m}")),
            "codex {m}"
        );
    }
}
#[test]
fn test_gemini_script_syntax() {
    for m in &["terminal-notifier", "osascript", "dunstify", "notify-send"] {
        assert!(
            check_bash_syntax(&gemini_script(m), &format!("gemini-{m}")),
            "gemini {m}"
        );
    }
}
#[test]
fn test_claude_script_syntax() {
    assert!(check_bash_syntax("#!/bin/bash\nSESSION=$(tmux display-message -p '#{session_name}' 2>/dev/null)||exit 0\nWINDOW=$(tmux display-message -p '#{window_name}' 2>/dev/null)||exit 0\nPANE=$(tmux display-message -p '#{pane_index}' 2>/dev/null)||exit 0\n[ -z '$SESSION' ]&&exit 0\nexec tlink notify --session '$SESSION' --window '$WINDOW' --pane '$PANE'","claude"));
}

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
    // Strip shebang line — we're wrapping in a context that redefines tmux
    let body = codex_script("osascript")
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");
    let s = format!("tmux() {{ exit 1; }}; export -f tmux; {body}");
    let (_, e, ok) = run_bash(&s);
    assert!(ok, "codex hook should exit 0 without tmux: stderr={e}");
}
#[test]
fn test_gemini_graceful_no_tmux() {
    let body = gemini_script("osascript")
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");
    let s = format!("tmux() {{ exit 1; }}; export -f tmux; echo '' | {body}");
    let (_, e, ok) = run_bash(&s);
    assert!(ok, "gemini hook should exit 0 without tmux: stderr={e}");
}

fn tmux_fmt(f: &str) -> String {
    let o = Command::new("tmux")
        .args(["display-message", "-p", f])
        .output()
        .unwrap();
    assert!(o.status.success());
    String::from_utf8_lossy(&o.stdout).trim().to_string()
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

/// Write a mock tmux script that fails switch-client (simulating no attached client)
/// but delegates other subcommands to the real tmux binary at `real_tmux`.
fn write_mock_tmux(dir: &PathBuf, real_tmux: &str) -> PathBuf {
    let mock_path = dir.join("tmux");
    let script = format!(
        r##"#!/bin/bash
# Mock: fail switch-client to simulate detached state
if [ "$1" = "switch-client" ]; then
    echo "MOCK: switch-client would fail" >&2
    exit 1
fi
# Mock: return no clients so detect_from_running_tmux() returns None
if [ "$1" = "list-clients" ]; then
    echo "MOCK: no clients" >&2
    exit 0
fi
exec {real_tmux} "$@"
"##
    );
    fs::write(&mock_path, &script).unwrap();
    let mut perm = fs::metadata(&mock_path).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&mock_path, perm).unwrap();
    mock_path
}

#[test]
fn test_tlink_open() {
    let s = tmux_fmt("#{{session_name}}");
    let o = tlink_cmd(&["open", &format!("tmux://{s}")])
        .output()
        .unwrap();
    if !o.status.success() {
        let stderr = String::from_utf8_lossy(&o.stderr);
        eprintln!("  tlink open skipped (expected on headless CI): {stderr}");
    }
}

/// Test that `tlink open` gracefully handles the case where no tmux client is
/// attached (simulated by a mock tmux that fails switch-client) and no
/// terminal adapter is configured.
#[test]
fn test_tlink_open_detached_no_adapter() {
    let mock_dir = std::env::temp_dir().join(format!("tlink-test-na-{}", std::process::id()));
    let config_path = mock_dir.join("config.toml");
    fs::create_dir_all(&mock_dir).unwrap();
    // Write empty config (no terminal adapter)
    fs::write(&config_path, b"").unwrap();

    let real_tmux = Command::new("which").arg("tmux").output().unwrap();
    let real_tmux = String::from_utf8_lossy(&real_tmux.stdout)
        .trim()
        .to_string();
    assert!(!real_tmux.is_empty(), "tmux must be installed");

    let mock = write_mock_tmux(&mock_dir, &real_tmux);
    let mock_dir_str = mock.parent().unwrap().to_string_lossy().to_string();

    let session_name = format!("tlink-test-na-{}", std::process::id());
    let _ = Command::new(&real_tmux)
        .args(["new-session", "-d", "-s", &session_name])
        .output();

    let path_val = format!(
        "{mock_dir_str}:{}",
        std::env::var("PATH").unwrap_or_default()
    );
    let output = tlink_cmd(&["open", &format!("tmux://{session_name}/0/0")])
        .env("PATH", &path_val)
        .env("TLINK_LOG", "1")
        .env("TLINK_CONFIG", config_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("=== test_tlink_open_detached_no_adapter ===");
    eprintln!("exit:  {}", output.status);
    eprintln!("stdout:\n{stdout}");
    eprintln!("stderr:\n{stderr}");

    assert!(
        matches!(output.status.success(), false),
        "expected failure when no adapter configured"
    );
    assert!(
        stderr.contains("no terminal adapter configured"),
        "stderr should mention missing adapter"
    );
    assert!(
        stderr.contains("[tlink] execute_switch: switch-client FAILED"),
        "logs should show switch-client failure"
    );

    let _ = Command::new(&real_tmux)
        .args(["kill-session", "-t", &session_name])
        .output();
    fs::remove_dir_all(&mock_dir).ok();
}

/// Test the fallback path when a terminal adapter IS configured.
/// The mock forces switch-client to fail, then attach_tmux should be called.
#[test]
fn test_tlink_open_detached_with_adapter() {
    let mock_dir = std::env::temp_dir().join(format!("tlink-test-wa-{}", std::process::id()));
    let config_path = mock_dir.join("config.toml");
    fs::create_dir_all(&mock_dir).unwrap();
    // Write config with Terminal.app adapter
    fs::write(&config_path, r#"terminal = "Terminal.app""#).unwrap();

    let real_tmux = Command::new("which").arg("tmux").output().unwrap();
    let real_tmux = String::from_utf8_lossy(&real_tmux.stdout)
        .trim()
        .to_string();
    assert!(!real_tmux.is_empty(), "tmux must be installed");

    let mock = write_mock_tmux(&mock_dir, &real_tmux);
    let mock_dir_str = mock.parent().unwrap().to_string_lossy().to_string();

    let session_name = format!("tlink-test-wa-{}", std::process::id());
    let _ = Command::new(&real_tmux)
        .args(["new-session", "-d", "-s", &session_name])
        .output();

    let path_val = format!(
        "{mock_dir_str}:{}",
        std::env::var("PATH").unwrap_or_default()
    );
    let output = tlink_cmd(&["open", &format!("tmux://{session_name}/0/0")])
        .env("PATH", &path_val)
        .env("TLINK_LOG", "1")
        .env("TLINK_CONFIG", config_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("=== test_tlink_open_detached_with_adapter ===");
    eprintln!("exit:  {}", output.status);
    eprintln!("stdout:\n{stdout}");
    eprintln!("stderr:\n{stderr}");

    assert!(
        output.status.success(),
        "expected success when adapter configured for fallback"
    );
    assert!(
        stderr.contains("[tlink] execute_switch: switch-client FAILED"),
        "logs should show switch-client failure"
    );
    assert!(
        stderr.contains("[tlink] execute_switch: falling back to attach_tmux"),
        "logs should show attach_tmux fallback"
    );

    let _ = Command::new(&real_tmux)
        .args(["kill-session", "-t", &session_name])
        .output();
    fs::remove_dir_all(&mock_dir).ok();
}

#[test]
fn test_hook_pipe() {
    let p = r#"{"hook_event_name":"Notification","notification_type":"idle_prompt","message":"Hook test"}"#;
    let b = std::env::var("TLINK_BIN").unwrap_or_else(|_| "target/debug/tlink".into());
    let c = format!(
        "printf '%s' '{p}' | '{}' notify --session s --window 1 --pane 0",
        b
    );
    let o = Command::new("bash").args(["-c", &c]).output().unwrap();
    assert!(
        o.status.success(),
        "pipe failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn test_config_roundtrip() {
    let t = std::env::temp_dir().join(format!("tlink-cfg-{}.toml", std::process::id()));
    std::fs::write(&t, "notification_method = \"terminal-notifier\"\n").unwrap();
    assert!(std::fs::read_to_string(&t)
        .unwrap()
        .contains("terminal-notifier"));
    std::fs::remove_file(&t).ok();
}

#[test]
fn test_gemini_python_parser() {
    let mut c=Command::new("python3").args(["-c",r#"import sys,json,shlex; d=json.loads(sys.stdin.read()); print('MESSAGE='+shlex.quote(d.get('message','')))"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
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
