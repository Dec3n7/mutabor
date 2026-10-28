//! Runs conversions as ffmpeg child processes and reports progress to the
//! toast window through events.

use crate::recipes::{self, Options, PlanError, Post, Probe};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

pub const TOAST: &str = "toast";

struct Running {
    child: Child,
    cancelled: bool,
}

#[derive(Default)]
pub struct Jobs {
    next_id: AtomicU64,
    running: Mutex<HashMap<u64, Running>>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum Event {
    Started { id: u64, name: String },
    Progress { id: u64, percent: u8 },
    Done { id: u64, output: String, before: u64, after: u64 },
    Failed { id: u64, message: String },
    Cancelled { id: u64 },
}

/// Sidecars are copied next to the main executable by the Tauri bundler.
fn ffmpeg_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })
}

fn ffmpeg() -> Command {
    let mut cmd = Command::new(ffmpeg_path());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// `video.mp4` + ("_compressed", "mp4") -> `video_compressed.mp4`, or
/// `video_compressed (1).mp4` when taken. Never returns the input itself.
fn output_path(input: &Path, suffix: &str, ext: &str) -> PathBuf {
    let dir = input.parent().unwrap_or(Path::new("."));
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let base = format!("{stem}{suffix}");
    (0..)
        .map(|n| match n {
            0 => dir.join(format!("{base}.{ext}")),
            n => dir.join(format!("{base} ({n}).{ext}")),
        })
        .find(|p| p != input && !p.exists())
        .expect("unbounded range")
}

fn probe(input: &Path) -> Probe {
    // `ffmpeg -i` without an output exits with an error but prints the banner.
    match ffmpeg().arg("-hide_banner").arg("-i").arg(input).output() {
        Ok(out) => Probe::parse(&String::from_utf8_lossy(&out.stderr)),
        Err(_) => Probe::default(),
    }
}

fn show_toast(app: &AppHandle) {
    let Some(win) = app.get_webview_window(TOAST) else { return };
    if let (Ok(Some(m)), Ok(size)) = (win.primary_monitor(), win.outer_size()) {
        // Bottom-right corner, above a typical taskbar.
        let x = m.position().x + m.size().width as i32 - size.width as i32 - 16;
        let y = m.position().y + m.size().height as i32 - size.height as i32 - 64;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    let _ = win.show();
}

impl Jobs {
    /// Starts one job per input. Errors only when no job could start.
    pub fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        action: &str,
        inputs: Vec<String>,
        opts: &Options,
    ) -> Result<(), String> {
        let mut started = 0;
        let mut last_err = String::new();
        for input in inputs {
            match self.start_one(app, action, PathBuf::from(input), opts) {
                Ok(()) => started += 1,
                Err(e) => last_err = e,
            }
        }
        if started == 0 { Err(last_err) } else { Ok(()) }
    }

    fn start_one(
        self: &Arc<Self>,
        app: &AppHandle,
        action: &str,
        input: PathBuf,
        opts: &Options,
    ) -> Result<(), String> {
        let ext = input
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let probe = probe(&input);
        let plan = recipes::plan(action, &ext, probe, opts).map_err(|e| match e {
            PlanError::NotReady => "not_ready".to_string(),
            PlanError::NeedsDuration => "no_duration".to_string(),
            PlanError::NoAudio => "no_audio".to_string(),
            PlanError::BadRange => "bad_range".to_string(),
        })?;
        let duration = plan.expected.or(probe.duration);
        let output = output_path(&input, &plan.suffix, &plan.ext);
        // With a post step ffmpeg writes an intermediate file next to the result.
        let written = match plan.post {
            Some(Post::JpegToPdf(_)) => output.with_extension("mutabor-tmp.jpg"),
            None => output.clone(),
        };

        let mut child = ffmpeg()
            .args(["-hide_banner", "-nostdin", "-n"])
            .args(&plan.input_args)
            .arg("-i")
            .arg(&input)
            .args(&plan.args)
            .args(["-progress", "pipe:1", "-nostats"])
            .arg(&written)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg: {e}"))?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.running.lock().unwrap().insert(id, Running { child, cancelled: false });

        let name = output.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let _ = app.emit_to(TOAST, "job", Event::Started { id, name });
        show_toast(app);

        // Drain stderr so ffmpeg never blocks on a full pipe; keep the tail for errors.
        let tail = Arc::new(Mutex::new(VecDeque::<String>::new()));
        let tail_w = tail.clone();
        let stderr_thread = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut t = tail_w.lock().unwrap();
                if t.len() == 8 {
                    t.pop_front();
                }
                t.push_back(line);
            }
        });

        let jobs = self.clone();
        let app = app.clone();
        std::thread::spawn(move || {
            let mut last = 0u8;
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                // `out_time_us` and the misnamed `out_time_ms` are both microseconds.
                let Some(us) = line
                    .strip_prefix("out_time_us=")
                    .or_else(|| line.strip_prefix("out_time_ms="))
                    .and_then(|v| v.parse::<f64>().ok())
                else {
                    continue;
                };
                if let Some(total) = duration.filter(|d| *d > 0.0) {
                    let percent = (us / 1e6 / total * 100.0).clamp(0.0, 99.0) as u8;
                    if percent > last {
                        last = percent;
                        let _ = app.emit_to(TOAST, "job", Event::Progress { id, percent });
                    }
                }
            }
            let _ = stderr_thread.join();

            let Some(Running { mut child, cancelled }) = jobs.running.lock().unwrap().remove(&id) else {
                return;
            };
            let mut ok = child.wait().map(|s| s.success()).unwrap_or(false);
            if let (true, false, Some(Post::JpegToPdf(page))) = (ok, cancelled, plan.post) {
                if let Err(e) = crate::pdf::convert(&written, &output, page) {
                    tail.lock().unwrap().push_back(format!("pdf: {e}"));
                    ok = false;
                }
            }
            if written != output {
                let _ = std::fs::remove_file(&written);
            }
            let event = if ok && !cancelled {
                let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                Event::Done {
                    id,
                    output: output.to_string_lossy().into_owned(),
                    before: size(&input),
                    after: size(&output),
                }
            } else {
                // Never leave a half-written file behind.
                let _ = std::fs::remove_file(&output);
                if cancelled {
                    Event::Cancelled { id }
                } else {
                    let message = tail.lock().unwrap().back().cloned().unwrap_or_default();
                    eprintln!("job {id} failed: {:?}", tail.lock().unwrap());
                    Event::Failed { id, message }
                }
            };
            let _ = app.emit_to(TOAST, "job", event);
        });
        Ok(())
    }

    pub fn cancel(&self, id: u64) {
        if let Some(job) = self.running.lock().unwrap().get_mut(&id) {
            job.cancelled = true;
            let _ = job.child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names() {
        let dir = std::env::temp_dir().join("mutabor-test-names");
        let _ = std::fs::create_dir_all(&dir);
        let input = dir.join("clip.mp4");
        std::fs::write(&input, b"x").unwrap();
        // Same extension and no suffix must not overwrite the source.
        assert_eq!(output_path(&input, "", "mp4"), dir.join("clip (1).mp4"));
        assert_eq!(output_path(&input, "_compressed", "mp4"), dir.join("clip_compressed.mp4"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
