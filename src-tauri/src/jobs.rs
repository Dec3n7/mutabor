//! Runs conversions as child processes (ffmpeg, MuPDF, Pandoc) and engine
//! downloads, and reports their progress to the toast window through events.

use crate::recipes::{self, Options, PlanError, Plan, Post, Probe, Tool};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

pub const TOAST: &str = "toast";

#[derive(Default)]
struct Running {
    /// The process of the current step, if one is running.
    child: Option<Child>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct Jobs {
    next_id: AtomicU64,
    running: Mutex<HashMap<u64, Running>>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum Event {
    /// `steady`: no percentages will follow, show a busy bar.
    Started { id: u64, name: String, steady: bool },
    Progress { id: u64, percent: u8 },
    Done { id: u64, output: String, before: u64, after: u64 },
    /// An engine finished downloading.
    Installed { id: u64 },
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

fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn ffmpeg() -> Command {
    command(&ffmpeg_path())
}

/// `video.mp4` + ("_compressed", "mp4") -> `video_compressed.mp4`, or
/// `video_compressed (1).mp4` when taken. An empty `ext` names a folder.
/// Never returns the input itself.
fn output_path(input: &Path, suffix: &str, ext: &str) -> PathBuf {
    let dir = input.parent().unwrap_or(Path::new("."));
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let base = format!("{stem}{suffix}");
    let dot = if ext.is_empty() { String::new() } else { format!(".{ext}") };
    (0..)
        .map(|n| match n {
            0 => dir.join(format!("{base}{dot}")),
            n => dir.join(format!("{base} ({n}){dot}")),
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

fn size_of(p: &Path) -> u64 {
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p)
            .map(|d| d.flatten().map(|e| size_of(&e.path())).sum())
            .unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
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

fn plan_error(e: PlanError) -> String {
    match e {
        PlanError::NotReady => "not_ready",
        PlanError::NeedsDuration => "no_duration",
        PlanError::NoAudio => "no_audio",
        PlanError::BadRange => "bad_range",
    }
    .into()
}

/// One process of a job.
struct Step {
    cmd: Command,
    /// Parse ffmpeg's `-progress` output for percentages.
    ffmpeg: bool,
}

/// Fills `{in}`, `{out}`, `{inputs}`, `{page}` and `{page3}` in a tool's arguments.
fn expand(args: &[String], inputs: &[PathBuf], out: &Path, page: Option<u32>) -> Vec<String> {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let mut v = Vec::new();
    for a in args {
        if a == "{inputs}" {
            v.extend(inputs.iter().map(|p| path(p)));
            continue;
        }
        let mut a = a.replace("{in}", &path(&inputs[0])).replace("{out}", &path(out));
        if let Some(n) = page {
            a = a.replace("{page3}", &format!("{n:03}")).replace("{page}", &n.to_string());
        }
        v.push(a);
    }
    v
}

/// Page count of a PDF, via `mutool show <pdf> trailer/Root/Pages/Count`.
fn pdf_pages(mutool: &Path, pdf: &Path) -> Option<u32> {
    let out = command(mutool).arg("show").arg(pdf).arg("trailer/Root/Pages/Count").output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

impl Jobs {
    /// Starts one job per input (merging is one job for all of them).
    /// Errors only when no job could start.
    pub fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        action: &str,
        inputs: Vec<String>,
        opts: &Options,
    ) -> Result<(), String> {
        let mut inputs: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
        if action == "merge" {
            // Explorer reports the selection in click order; file names are more predictable.
            inputs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
            return self.start_one(app, action, inputs, opts);
        }
        let mut started = 0;
        let mut last_err = String::new();
        for input in inputs {
            match self.start_one(app, action, vec![input], opts) {
                Ok(()) => started += 1,
                Err(e) => last_err = e,
            }
        }
        if started == 0 { Err(last_err) } else { Ok(()) }
    }

    fn start_one(self: &Arc<Self>, app: &AppHandle, action: &str, inputs: Vec<PathBuf>, opts: &Options) -> Result<(), String> {
        let input = inputs[0].clone();
        let ext = input
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let kind = crate::files::kind_of(&ext);
        let media = matches!(kind, crate::files::Kind::Video | crate::files::Kind::Audio);
        // Probing runs ffmpeg; only audio and video need it.
        let probe = if media { probe(&input) } else { Probe::default() };
        let plan = recipes::plan(action, &ext, probe, opts).map_err(plan_error)?;
        let program = match plan.tool.engine() {
            None => ffmpeg_path(),
            Some((engine, program)) => crate::engines::program(app, engine, program)
                .ok_or_else(|| format!("engine_missing:{engine}"))?,
        };
        let duration = plan.expected.or(probe.duration);
        let output = output_path(&input, &plan.suffix, if plan.dir { "" } else { &plan.ext });
        // With a post step ffmpeg writes an intermediate file next to the result.
        let written = match plan.post {
            Some(Post::JpegToPdf(_)) => output.with_extension("mutabor-tmp.jpg"),
            _ => output.clone(),
        };
        if plan.dir {
            std::fs::create_dir(&output).map_err(|e| e.to_string())?;
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap().insert(id, Running { child: None, cancelled: cancelled.clone() });
        let name = output.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let steady = plan.tool != Tool::Ffmpeg && !plan.per_page;
        let _ = app.emit_to(TOAST, "job", Event::Started { id, name, steady });
        show_toast(app);

        let jobs = self.clone();
        let app = app.clone();
        std::thread::spawn(move || {
            let result = jobs.work(&app, id, &plan, &program, &inputs, &written, duration);
            let ok = result.is_ok() && !cancelled.load(Ordering::Relaxed);
            let mut result = result;
            if ok {
                if let Some(Post::JpegToPdf(page)) = plan.post {
                    result = crate::pdf::convert(&written, &output, page);
                }
            }
            if written != output {
                let _ = std::fs::remove_file(&written);
            }
            jobs.running.lock().unwrap().remove(&id);

            let event = match (result, cancelled.load(Ordering::Relaxed)) {
                (Ok(()), false) => Event::Done {
                    id,
                    output: output.to_string_lossy().into_owned(),
                    before: inputs.iter().map(|p| size_of(p)).sum(),
                    after: size_of(&output),
                },
                (result, cancelled) => {
                    // Never leave a half-written result behind.
                    if plan.dir {
                        let _ = std::fs::remove_dir_all(&output);
                    } else {
                        let _ = std::fs::remove_file(&output);
                    }
                    if cancelled {
                        Event::Cancelled { id }
                    } else {
                        let message = result.err().unwrap_or_default();
                        eprintln!("job {id} failed: {message}");
                        Event::Failed { id, message }
                    }
                }
            };
            let _ = app.emit_to(TOAST, "job", event);
        });
        Ok(())
    }

    /// Builds and runs the job's processes in order; Err carries the last
    /// lines the failing program printed.
    fn work(
        &self,
        app: &AppHandle,
        id: u64,
        plan: &Plan,
        program: &Path,
        inputs: &[PathBuf],
        written: &Path,
        duration: Option<f64>,
    ) -> Result<(), String> {
        let mut steps = Vec::new();
        match plan.tool {
            Tool::Ffmpeg => {
                let mut cmd = ffmpeg();
                cmd.args(["-hide_banner", "-nostdin", "-n"])
                    .args(&plan.input_args)
                    .arg("-i")
                    .arg(&inputs[0])
                    .args(&plan.args)
                    .args(["-progress", "pipe:1", "-nostats"])
                    .arg(written);
                steps.push(Step { cmd, ffmpeg: true });
            }
            _ => {
                let pages: Vec<Option<u32>> = if plan.per_page {
                    let n = pdf_pages(program, &inputs[0]).ok_or("couldn't count the PDF's pages")?;
                    (1..=n).map(Some).collect()
                } else {
                    vec![None]
                };
                for page in pages {
                    let mut cmd = command(program);
                    cmd.args(expand(&plan.args, inputs, written, page));
                    // Relative links (pictures in Markdown) resolve next to the input.
                    if let Some(dir) = inputs[0].parent() {
                        cmd.current_dir(dir);
                    }
                    steps.push(Step { cmd, ffmpeg: false });
                }
            }
        }
        if let Some(Post::PngPagesToJpg(q)) = plan.post {
            let mut cmd = ffmpeg();
            cmd.args(["-hide_banner", "-nostdin", "-n", "-i"])
                .arg(written.join("page-%03d.png"))
                .args(["-q:v", q.pick("2", "3", "6"), "-start_number", "1"])
                .arg(written.join("page-%03d.jpg"));
            steps.push(Step { cmd, ffmpeg: false });
        }

        let total = steps.len();
        for (i, step) in steps.into_iter().enumerate() {
            self.run_step(app, id, step, duration)?;
            if total > 1 {
                let _ = app.emit_to(TOAST, "job", Event::Progress { id, percent: ((i + 1) * 100 / total).min(99) as u8 });
            }
        }
        if matches!(plan.post, Some(Post::PngPagesToJpg(_))) {
            for e in std::fs::read_dir(written).map_err(|e| e.to_string())?.flatten() {
                if e.path().extension().is_some_and(|x| x == "png") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    fn run_step(&self, app: &AppHandle, id: u64, mut step: Step, duration: Option<f64>) -> Result<(), String> {
        let mut child = step
            .cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        {
            let mut running = self.running.lock().unwrap();
            let Some(r) = running.get_mut(&id) else { return Err("cancelled".into()) };
            if r.cancelled.load(Ordering::Relaxed) {
                let _ = child.kill();
                return Err("cancelled".into());
            }
            r.child = Some(child);
        }

        // Drain stderr so the program never blocks on a full pipe; keep the tail for errors.
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

        let mut last = 0u8;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !step.ffmpeg {
                continue;
            }
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

        let child = self.running.lock().unwrap().get_mut(&id).and_then(|r| r.child.take());
        let ok = child.is_some_and(|mut c| c.wait().is_ok_and(|s| s.success()));
        if ok {
            Ok(())
        } else {
            let tail = tail.lock().unwrap();
            Err(tail.iter().cloned().collect::<Vec<_>>().join("\n"))
        }
    }

    /// Downloads an engine, showing progress in the toast. Blocks until done.
    pub fn install_engine(&self, app: &AppHandle, engine: &str, title: &str) -> Result<(), String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap().insert(id, Running { child: None, cancelled: cancelled.clone() });
        let _ = app.emit_to(TOAST, "job", Event::Started { id, name: title.into(), steady: false });
        show_toast(app);
        let result = crate::engines::install(app, engine, &cancelled, |percent| {
            let _ = app.emit_to(TOAST, "job", Event::Progress { id, percent });
        });
        self.running.lock().unwrap().remove(&id);
        let event = match &result {
            Ok(()) => Event::Installed { id },
            Err(_) if cancelled.load(Ordering::Relaxed) => Event::Cancelled { id },
            Err(e) => Event::Failed { id, message: e.clone() },
        };
        let _ = app.emit_to(TOAST, "job", event);
        result
    }

    pub fn cancel(&self, id: u64) {
        if let Some(job) = self.running.lock().unwrap().get_mut(&id) {
            job.cancelled.store(true, Ordering::Relaxed);
            if let Some(child) = job.child.as_mut() {
                let _ = child.kill();
            }
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
        assert_eq!(output_path(&input, "_pages", ""), dir.join("clip_pages"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs the MuPDF and Pandoc recipes on generated samples.
    /// Needs `npm run tools -- --optional` first: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn tools_run_on_real_binaries() {
        let bin = |name: &str| {
            std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries"))
                .expect("run `npm run tools -- --optional` first")
                .flatten()
                .map(|e| e.path())
                .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(&format!("{name}-")))
                .unwrap_or_else(|| panic!("{name} sidecar"))
        };
        let (ffmpeg, mutool, pandoc) = (bin("ffmpeg"), bin("mutool"), bin("pandoc"));
        let dir = std::env::temp_dir().join("mutabor-tools");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ok = |cmd: &mut Command| {
            let out = cmd.output().unwrap();
            assert!(out.status.success(), "{cmd:?}\n{}", String::from_utf8_lossy(&out.stderr));
        };

        // Our own image-to-PDF writer must produce a PDF MuPDF accepts.
        let jpg = dir.join("photo.jpg");
        ok(Command::new(&ffmpeg).args(["-v", "error", "-f", "lavfi", "-i", "testsrc=size=640x480", "-frames:v", "1"]).arg(&jpg));
        let one = dir.join("one.pdf");
        crate::pdf::convert(&jpg, &one, recipes::Page::A4).unwrap();
        assert_eq!(pdf_pages(&mutool, &one), Some(1));
        let two = dir.join("two.pdf");
        ok(Command::new(&mutool).args(["merge", "-o"]).arg(&two).arg(&one).arg(&one));
        assert_eq!(pdf_pages(&mutool, &two), Some(2));

        let run = |action: &str, inputs: &[PathBuf]| -> PathBuf {
            let ext = inputs[0].extension().unwrap().to_string_lossy().into_owned();
            let p = recipes::plan(action, &ext, Probe::default(), &Options::default()).unwrap();
            let program = if p.tool == Tool::Mutool { &mutool } else { &pandoc };
            let out = output_path(&inputs[0], &format!("_{action}{}", p.suffix), if p.dir { "" } else { &p.ext });
            if p.dir {
                std::fs::create_dir(&out).unwrap();
            }
            let pages: Vec<Option<u32>> = if p.per_page {
                (1..=pdf_pages(program, &inputs[0]).unwrap()).map(Some).collect()
            } else {
                vec![None]
            };
            for page in pages {
                ok(Command::new(program).args(expand(&p.args, inputs, &out, page)));
            }
            assert!(size_of(&out) > 0, "{action}: empty result");
            out
        };
        let count = |d: &Path| std::fs::read_dir(d).unwrap().count();
        assert_eq!(count(&run("pages_png", &[two.clone()])), 2);
        assert_eq!(count(&run("split", &[two.clone()])), 2);
        assert_eq!(pdf_pages(&mutool, &run("merge", &[one.clone(), two.clone()])), Some(3));
        run("extract_text", &[two.clone()]);
        run("compress", &[two.clone()]);

        let md = dir.join("book.md");
        std::fs::write(&md, "# Глава 1\n\nПривет, **мир**.\n").unwrap();
        for action in ["to_html", "to_epub", "to_fb2", "to_txt"] {
            run(action, &[md.clone()]);
        }
        let epub = dir.join("book_to_epub.epub");
        let back = run("to_md", &[epub]);
        assert!(std::fs::read_to_string(back).unwrap().contains("**мир**"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn placeholders() {
        let inputs = [PathBuf::from("a.pdf"), PathBuf::from("b.pdf")];
        let args: Vec<String> = ["merge", "-o", "{out}/p-{page3}.pdf", "{in}", "{page}", "{inputs}"]
            .map(String::from)
            .to_vec();
        let v = expand(&args, &inputs, Path::new("out"), Some(7));
        assert_eq!(v, ["merge", "-o", "out/p-007.pdf", "a.pdf", "7", "a.pdf", "b.pdf"]);
    }
}
