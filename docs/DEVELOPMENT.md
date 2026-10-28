# Mutabor for developers

The technical side: how Mutabor is built, what it touches on your system, and
how to check that a download really comes from this repository. The
[README](../README.md) is the user-facing overview.

## Building from source

You need [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 20+ and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```bash
npm install
npm run tools          # fetches ffmpeg, checked against tools/manifest.json
npm run tauri dev      # run in development
npm run tauri build    # build an installer for your system
```

To work on the menu in a plain browser, without Rust: `npm run dev` and open
`http://localhost:1420/?demo=video` (also `image`, `mixed`, `pdf`, `book`,
`clipboard`, `none`; add `&options=<action>` to open an action's options and
`&lang=<code>` to switch the language). `toast.html` and `settings.html` have
demo data too.

## Tests

```bash
npm run tools -- --optional   # ffmpeg, MuPDF and Pandoc for the tests
cd src-tauri
cargo test                    # fast unit tests
cargo test -- --ignored       # every recipe on the real programs
npm run check-locales         # every language file against English
```

CI runs the same on Windows, macOS and Linux for every push.

## How it works

- **Tauri 2** app: Rust in `src-tauri/src`, the menu, toast and settings
  windows in plain TypeScript in `src`.
- **Recipes** (`recipes.rs`) turn a menu action and its options into a command
  line; they are pure functions with tests. **Jobs** (`jobs.rs`) run them as
  child processes in a queue with a per-kind limit, report progress to the
  toast and clean up partial results.
- **Engines.** ffmpeg 9.0.2 is bundled as a sidecar called `mutabor-ffmpeg`
  (so the Linux package does not clash with a system ffmpeg). SVG is drawn by
  [resvg](https://github.com/linebender/resvg), compiled in. MuPDF and Pandoc
  are optional and downloaded on first use:

  | Engine | Version | License | Download | Systems |
  | --- | --- | --- | --- | --- |
  | [MuPDF](https://mupdf.com) | 1.28.5 | AGPL-3.0 | 90 MB | Windows (no official macOS or Linux build) |
  | [Pandoc](https://pandoc.org) | 3.13 | GPL-2.0-or-later | 27–43 MB | Windows, macOS, Linux |

- **Third-party binaries are never committed.** Their source URL, version and
  SHA-256 live in [`tools/manifest.json`](../tools/manifest.json). The build
  script and the app itself refuse an archive whose hash does not match.
  ffmpeg comes from [Gyan.dev](https://www.gyan.dev/ffmpeg/builds/) on Windows
  and [Martin Riedl's builds](https://ffmpeg.martin-riedl.de/) on macOS and
  Linux, both linked from ffmpeg.org.

## What Mutabor does on your system

- Registers the global hotkey (Ctrl+Alt+C, ⌘⌥C on macOS) and any action
  hotkeys you set.
- Shows an icon in the tray (the menu bar on macOS).
- **Windows:** reads the selection of the active Explorer window through the
  standard `Shell.Application` COM object, and the clipboard through
  PowerShell when nothing is selected.
- **macOS:** asks Finder for its selection with AppleScript; macOS asks once
  for permission to control Finder.
- **Network:** engines are downloaded only when you press "Download", and
  only from the address in `tools/manifest.json` (GitHub for Pandoc,
  mupdf.com for MuPDF). Once a day, unless turned off in Settings, the app
  reads `api.github.com/repos/Dec3n7/mutabor/releases/latest` to see whether a
  newer version is out (`src-tauri/src/updates.rs`); nothing else is sent. No
  telemetry.
- **Files:** settings in `settings.json` in the OS config folder
  (`%APPDATA%\io.github.dec3n7.mutabor` on Windows), engines in
  `%LOCALAPPDATA%\io.github.dec3n7.mutabor\engines`. Results are written next
  to the original under a new name; the original is never changed. Pictures
  pasted from the clipboard are saved in `Pictures\Mutabor`.

## Releases and verifying a download

Releases are built only by the public [Release workflow](../.github/workflows/release.yml)
when a `v*` tag is pushed. Each release has a `SHA256SUMS.txt`, and every file
has a build provenance attestation. To check a file you downloaded:

```bash
gh attestation verify Mutabor_0.1.0_x64-setup.exe --repo Dec3n7/mutabor
```

or compare its SHA-256 with `SHA256SUMS.txt`.

The builds are not signed with a paid certificate yet, so Windows SmartScreen
and macOS Gatekeeper warn on first start. macOS builds carry an ad-hoc
signature; if macOS calls the app "damaged", run
`xattr -dr com.apple.quarantine /Applications/Mutabor.app`.

## Platform status

- **Windows:** everything works.
- **macOS:** no clipboard as a file source yet; no PDF actions (no MuPDF build).
- **Linux:** the menu cannot see the file manager's selection (Nautilus,
  Dolphin and others share no API for it), no clipboard as a source, no PDF
  actions; the global hotkey needs X11.

## Translations

Each language is one file in [`src/locales`](../src/locales) with the same
keys as `en.ts`. To add one, copy `en.ts`, translate the values, list the
language in `src/i18n.ts` and run `npm run check-locales`. Fixes to existing
translations are very welcome.
