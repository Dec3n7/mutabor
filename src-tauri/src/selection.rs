//! Reads the files currently selected in the system file manager.
//!
//! Called right after the global hotkey fires, while the file manager
//! window is still in the foreground.

/// Returns absolute paths of the selected files, or an empty list.
pub fn selected_paths() -> Vec<String> {
    imp::selected_paths().unwrap_or_default()
}

pub(crate) fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(target_os = "windows")]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> isize;
    }

    /// Asks Explorer (Shell.Application COM) for the selection of the
    /// foreground window.
    // TODO: Explorer tabs share one HWND, so with several tabs open the
    // first tab that has a selection wins. Replace with native COM later.
    // TODO: selection on the desktop is not covered by Shell.Windows().
    pub fn selected_paths() -> Option<Vec<String>> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd == 0 {
            return None;
        }
        let script = format!(
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
             $sh=New-Object -ComObject Shell.Application; \
             foreach($w in $sh.Windows()){{ try {{ \
               if([long]$w.HWND -eq {hwnd}){{ \
                 $s=$w.Document.SelectedItems(); \
                 if($s.Count -gt 0){{ foreach($i in $s){{ $i.Path }}; break }} \
               }} }} catch {{}} }}"
        );
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        Some(super::lines(&out.stdout))
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    /// Asks Finder for its selection via AppleScript. The first call
    /// triggers the macOS "control Finder" permission prompt.
    pub fn selected_paths() -> Option<Vec<String>> {
        let out = Command::new("osascript")
            .args([
                "-e", "tell application \"Finder\" to set s to selection as alias list",
                "-e", "set out to \"\"",
                "-e", "repeat with f in s",
                "-e", "set out to out & POSIX path of f & linefeed",
                "-e", "end repeat",
                "-e", "return out",
            ])
            .output()
            .ok()?;
        Some(super::lines(&out.stdout))
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    // TODO: Linux has no common API; needs per-file-manager integration
    // (Nautilus, Dolphin). Until then the context menu is the entry point.
    pub fn selected_paths() -> Option<Vec<String>> {
        None
    }
}
