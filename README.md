<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" height="96" alt="Mutabor icon">
</p>

<h1 align="center">Mutabor</h1>

<p align="center">
  <b>English</b> · <a href="README.zh.md">中文</a> · <a href="README.ru.md">Русский</a> · <a href="README.ja.md">日本語</a> · <a href="README.es.md">Español</a>
</p>

<p align="center">
  Convert and tidy up files in two keystrokes.<br>
  Free, open source, and it never sends your files anywhere.
</p>

<p align="center">
  <a href="https://github.com/Dec3n7/mutabor/releases/latest"><b>Download for Windows, macOS or Linux</b></a>
</p>

---

## How it works

1. Select one or more files in Explorer or Finder.
2. Press **Ctrl + Alt + C** (on a Mac: **⌘ + ⌥ + C**).
3. A small menu appears next to the cursor. Pick what to do.

The result is saved next to the original under a new name. The original stays
untouched.

## What it can do

- **Shrink a video so it fits.** Discord, email or any limit you choose: 8, 10,
  25, 50 MB and more.
- **Change formats.** Video, music, photos, PDF and e-books. iPhone photos
  (HEIC) to JPG, voice messages to MP3, video to GIF, and so on.
- **Quick fixes.** Rotate or flip a photo, remove its location data, trim a
  video, take the sound out, speed it up, grab a still frame.
- **PDF.** Merge several into one, split into pages, turn pages into pictures,
  make the file smaller.
- **Many files at once.** Select a whole folder: they are processed one after
  another, a few at a time.
- **Straight into a chat.** Press "Copy" when it's done, then Ctrl + V in
  Telegram, Discord or anywhere else.
- **Screenshots too.** Copy a screenshot, press the hotkey, and save it as a
  file or convert it.
- **Your own shortcuts.** Give any action its own key, or chain several actions
  into one ("remove sound, then shrink to 8 MB").

Word, Excel and PowerPoint files are not supported.

## Install

Open the [latest release](https://github.com/Dec3n7/mutabor/releases/latest)
and download the file for your computer:

| Your computer | File to download |
| --- | --- |
| Windows 10 or 11 | `Mutabor_…_x64-setup.exe` |
| Mac with Apple chip (M1 and newer) | `Mutabor_…_aarch64.dmg` |
| Mac with Intel chip | `Mutabor_…_x64.dmg` |
| Linux | `Mutabor_…_amd64.AppImage` |

After installing, Mutabor sits quietly in the tray (on a Mac, in the menu bar
at the top). There is no window to open: just select files and press the
hotkey.

### "Windows protected your PC" or "can't be opened"

That is normal for a new free app: we haven't bought a code-signing
certificate yet, so your system doesn't know the publisher.

- **Windows:** click **More info**, then **Run anyway**.
- **Mac:** open **System Settings → Privacy & Security**, scroll down and click
  **Open Anyway**.

## Is it safe?

- **Your files stay on your computer.** Nothing is uploaded, there is no
  account and no tracking.
- **It goes online only when you ask.** Some PDF and e-book actions need an
  extra free component; Mutabor shows its size and downloads it only after you
  agree.
- **Everything is open.** The whole source code is in this repository, and the
  installers are built by GitHub in public, not on someone's computer.

## Good to know

- Press **Shift + Enter** on an action to change its settings: quality, size,
  time range.
- **Settings** are in the tray menu: change the hotkey, the language, how many
  files run at once, and more.
- The app speaks 13 languages and follows your system's language.
- **On a Mac,** copied screenshots and PDF actions don't work yet. **On
  Linux,** the menu can't see the files you selected yet, so it's for
  enthusiasts for now.

## Help out

Found a bug or have an idea? [Open an issue](https://github.com/Dec3n7/mutabor/issues).
Translation fixes are very welcome. Developers will find build instructions and
technical details in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

Mutabor is free software under the [GPL-3.0](LICENSE) license.

<sub>The name comes from Wilhelm Hauff's fairy tale "Caliph Stork": "Mutabor!"
is the magic word that transforms. In Latin it means "I shall be changed".</sub>
