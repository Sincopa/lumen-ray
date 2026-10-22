//! Login item as a per-user LaunchAgent. launchd reads it at the next login;
//! it is never loaded now, which would start a second Lumen immediately.
use crate::domain::errors::{BackendError, Result};
use std::path::{Path, PathBuf};
const LABEL: &str = "app.lumen.desktop";

fn agent() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|home| !home.is_empty())?;
    Some(
        PathBuf::from(home)
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
    )
}
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}
fn plist(executable: &Path) -> Option<String> {
    // XML 1.0 cannot carry control characters even escaped.
    let path = executable
        .to_str()
        .filter(|p| !p.chars().any(char::is_control))?;
    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>--minimized</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
	<key>ProcessType</key>
	<string>Interactive</string>
</dict>
</plist>
"#,
        escape(path)
    ))
}
/// A mounted disk image or a quarantined app's randomized copy is gone by the next login.
fn transient(executable: &Path) -> bool {
    executable.starts_with("/Volumes")
        || executable
            .components()
            .any(|part| part.as_os_str() == "AppTranslocation")
}
fn failed(message: &str) -> BackendError {
    BackendError::new("STORAGE_FAILED", message)
}
pub fn set_autostart(enabled: bool) -> Result<()> {
    let path = agent().ok_or_else(|| failed("Home folder is unavailable"))?;
    if enabled {
        let exe = std::env::current_exe().map_err(|_| failed("Executable path is unavailable"))?;
        if transient(&exe) {
            return Err(BackendError::new(
                "AUTOSTART_UNAVAILABLE",
                "Move Lumen to the Applications folder to open it at login",
            ));
        }
        let text = plist(&exe).ok_or_else(|| failed("Executable path is unavailable"))?;
        let parent = path
            .parent()
            .ok_or_else(|| failed("Failed to register autostart"))?;
        std::fs::create_dir_all(parent).map_err(|_| failed("Failed to access LaunchAgents"))?;
        crate::xray::updater::atomic_write(&path, text.as_bytes())
            .map_err(|_| failed("Failed to register autostart"))?;
    } else if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(failed("Failed to remove autostart"));
        }
    }
    Ok(())
}
/// On only if the agent starts this executable: a moved app needs re-registering.
pub fn is_autostart() -> bool {
    let (Some(path), Ok(exe)) = (agent(), std::env::current_exe()) else {
        return false;
    };
    std::fs::read_to_string(path).is_ok_and(|text| plist(&exe).is_some_and(|want| want == text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_plist_escapes_the_executable_path() {
        let text = plist(Path::new(
            "/Applications/A&B <\"Lumen\">'s.app/Contents/MacOS/Lumen",
        ))
        .unwrap();
        assert!(text.contains(
            "<string>/Applications/A&amp;B &lt;&quot;Lumen&quot;&gt;&apos;s.app/Contents/MacOS/Lumen</string>"
        ));
        assert!(text.contains("<string>--minimized</string>"));
        assert!(text.contains("<string>app.lumen.desktop</string>"));
        assert_eq!(plist(Path::new("/Applications/bad\nname")), None);
    }

    #[test]
    fn transient_locations_are_refused() {
        assert!(transient(Path::new(
            "/Volumes/Lumen/Lumen.app/Contents/MacOS/lumen"
        )));
        assert!(transient(Path::new(
            "/private/var/folders/x/T/AppTranslocation/0A1B/d/Lumen.app/Contents/MacOS/lumen"
        )));
        assert!(!transient(Path::new(
            "/Applications/Lumen.app/Contents/MacOS/lumen"
        )));
    }

    #[test]
    fn agent_plist_is_valid_for_plutil() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("agent.plist");
        std::fs::write(
            &file,
            plist(Path::new(
                "/Applications/Lumen & Co.app/Contents/MacOS/Lumen",
            ))
            .unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new("/usr/bin/plutil")
            .arg("-lint")
            .arg(&file)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }
}
