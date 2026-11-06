# Contributing to Mutabor

Thanks for helping. Bug reports, ideas and translation fixes are as welcome as
code: [open an issue](https://github.com/Dec3n7/mutabor/issues).

## What Mutabor is trying to be

The fastest way from "I selected a file" to "it's done", on your own computer.
We care about fewer key presses, tools that work offline, and promises anyone
can check. We don't chase the longest list of formats, so a change that adds a
format few people need, or a step to a common action, may be declined. Ask in
an issue first if you're planning something big.

## Getting started

[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) has what you need to install, how to
run the app, and how it's built. In short:

```bash
npm install
npm run tools
npm run tauri dev
```

To work on the menu without Rust, run `npm run dev` and open
`http://localhost:1420/?demo=video`.

## Before you open a pull request

```bash
npx tsc --noEmit
npm run check-locales
cd src-tauri
cargo test
```

`cargo test -- --ignored` runs every recipe on the real programs; it needs
`npm run tools -- --optional` first. CI runs all of this on Windows, macOS and
Linux.

## Common changes

**A new action**

1. Add its command line to `plan` in `src-tauri/src/recipes.rs`, with a test.
   Recipes are pure functions, so this needs no running app.
2. Add it to the list for its file kind in `src/actions.ts`, with search
   keywords in several languages.
3. Add its name to `src/locales/en.ts` and to every other language file.

**A new engine**

Add it to `tools/manifest.json` with its version, license, size, URL and
SHA-256. Never commit a third-party binary. A download from a new host also has
to be added to the list in the tests in `src-tauri/src/net.rs`, on purpose:
that list is what the privacy promise rests on. Add the engine to the table in
`docs/DEVELOPMENT.md`.

**A translation**

Each language is one file in `src/locales` with the same keys as `en.ts`. See
"Translations" in `docs/DEVELOPMENT.md`.

## Style

- Code comments say why, not what. Text for users is in plain words, without
  technical terms.
- Small commits with a message that says what changed for the user.
- By contributing, you agree that your work is released under the
  [GPL-3.0](LICENSE).
