# Speed benchmarks

Stage 0 of the development plan. These numbers are the baseline that later
changes are compared against. Measure by hand on the same machine each time.

## Scenarios

Use one test file per scenario, kept in the same folder, so runs are comparable.

| # | Scenario | Test file | Steps to start |
| --- | --- | --- | --- |
| 1 | Shrink a video for Discord (≤ 10 MB) | a 1-minute 1080p video | select file, open menu, choose the Discord preset |
| 2 | Remove GPS data from a photo | a JPEG with GPS tags | select file, open menu, choose the action |
| 3 | Rotate a photo | any JPEG | select file, open menu, choose rotate |
| 4 | Remove a background | a portrait photo | select file, open menu, choose background removal |
| 5 | Upscale a picture | a 500×500 PNG | select file, open menu, choose upscale |

Scenarios 4 and 5 need their engine already downloaded, otherwise the time
includes the download. Note it in the table if it did.

## How to measure

1. Select the test file in Explorer.
2. Press the hotkey (or the action's own hotkey, if one is set).
3. Start the timer on the key press. Stop it when the result file appears in
   the folder, or when the toast says it is done.
4. Count every key press and mouse click between step 1 and the result.
5. Repeat three times. Record the median.

## Results

| # | Scenario | Keys + clicks | Median time (s) | Notes |
| --- | --- | --- | --- | --- |
| 1 | Discord video | 5 keys (hotkey, `d` `i` `s`, Enter) | 8.4 (runs: 8.8, 8.4, 7.5) | Dev build (debug), menu → Discord preset, 40.8 → 9.2 MB. Keys counted from the menu code: "Discord" is the 11th row, so it is found by typing. |
| 2 | Remove GPS | 2 keys (hotkey, `9`) | 0.1 (runs: 0.2, 0.1, 0.1) | Dev build (debug). GPS tag verified gone in the output. Row 9 of the picture list, so a digit key picks it. |
| 3 | Rotate | 1 key + 1 click (hotkey, ↻ 90° button) | 0.1 (runs: 0.1, 0.1, 0.1) | Dev build (debug). Three `_rotated` outputs created. Rotation is a quick button above the list. |
| 4 | Remove background | 5 keys (hotkey, `r` `e` `m`, Enter) | 19.0 (runs: 19.4, 19.0, 18.7; earlier set 20.6, 19.0, 18.7) | Dev build (debug). Input was a cat picture that the user put in place of the portrait stand-in (not a portrait); rerun on a real portrait if the spec's input matters. Assumes the engine is already installed. |
| 5 | Upscale | 5 keys (hotkey, `u` `p` `s`, Enter) | 4.8 (runs: 5.6, 4.8, 4.7) | Dev build (debug). 500×500 → ×4 output, three `_x4` files created. Enter runs it with default options. |

Keys were counted from the code before stage 1 (the menu's search needs the
first letters typed, digits work only for rows 1–9). Counts assume a one-letter-
unique prefix; a full word takes more keys.

Date and version of the build:

Machine (CPU, GPU, disk):
