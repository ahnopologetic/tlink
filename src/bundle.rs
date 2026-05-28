use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";

pub fn bundle_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join("Applications/TmuxLink.app")
}

pub fn info_plist() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>TmuxLink</string>
    <key>CFBundleIdentifier</key>
    <string>com.tlink.handler</string>
    <key>CFBundleName</key>
    <string>TmuxLink</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSUIElement</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>tmux deeplink</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>tmux</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
"#
    .to_string()
}

pub fn swift_handler_source(tlink_bin: &str) -> String {
    format!(
        r#"import AppKit

class Handler: NSObject, NSApplicationDelegate {{
    func application(_ application: NSApplication, open urls: [URL]) {{
        for url in urls {{
            let task = Process()
            task.executableURL = URL(fileURLWithPath: "{tlink_bin}")
            task.arguments = ["open", url.absoluteString]
            try? task.run()
            task.waitUntilExit()
        }}
        NSApplication.shared.terminate(nil)
    }}
    func applicationDidFinishLaunching(_ notification: Notification) {{
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {{
            NSApplication.shared.terminate(nil)
        }}
    }}
}}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let delegate = Handler()
app.delegate = delegate
app.run()
"#
    )
}

pub fn find_tlink_binary() -> Result<String> {
    let out = Command::new("which").arg("tlink").output()?;
    if out.status.success() {
        let path = String::from_utf8(out.stdout)?.trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }
    let cargo_bin = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("home dir not found"))?
        .join(".cargo/bin/tlink");
    if cargo_bin.exists() {
        return Ok(cargo_bin.to_string_lossy().to_string());
    }
    bail!("tlink binary not found in PATH or ~/.cargo/bin. Run `cargo install --path .` first.")
}

pub fn create() -> Result<()> {
    let app = bundle_path();
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    std::fs::create_dir_all(&macos)?;

    let tlink_bin = find_tlink_binary()?;
    let swift_src = swift_handler_source(&tlink_bin);
    let swift_file = contents.join("handler.swift");
    std::fs::write(&swift_file, &swift_src)?;

    let handler_bin = macos.join("TmuxLink");
    let status = Command::new("swiftc")
        .args([swift_file.to_str().unwrap(), "-o", handler_bin.to_str().unwrap()])
        .status()?;
    if !status.success() {
        bail!("swiftc failed — is Xcode Command Line Tools installed? Run: xcode-select --install");
    }

    std::fs::write(contents.join("Info.plist"), info_plist())?;
    std::fs::remove_file(&swift_file).ok();

    Command::new(LSREGISTER)
        .args(["-f", app.to_str().unwrap()])
        .status()?;

    Ok(())
}

pub fn remove() -> Result<()> {
    let app = bundle_path();
    if app.exists() {
        std::fs::remove_dir_all(&app)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_plist_contains_tmux_scheme() {
        let plist = info_plist();
        assert!(plist.contains("<string>tmux</string>"));
        assert!(plist.contains("CFBundleURLSchemes"));
        assert!(plist.contains("com.tlink.handler"));
        assert!(plist.contains("<string>TmuxLink</string>"));
    }

    #[test]
    fn test_swift_source_references_tlink_path() {
        let src = swift_handler_source("/Users/bob/.cargo/bin/tlink");
        assert!(src.contains("/Users/bob/.cargo/bin/tlink"));
        assert!(src.contains("open urls: [URL]"));
        assert!(src.contains(r#"arguments = ["open", url.absoluteString]"#));
    }

    #[test]
    fn test_bundle_path_ends_with_tmuxlink() {
        let p = bundle_path();
        assert!(p.ends_with("Applications/TmuxLink.app"));
    }
}
