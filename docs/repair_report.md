# Kotoclip Repair Report

Updated: 2026-07-10

This file tracks the requested repairs. Each numbered item is committed separately.

| Item | Status | Summary | Commit |
| --- | --- | --- | --- |
| Baseline | Complete | Restored first analysis rendering, fixed overlapping grammar matches, measured virtual rows, constrained tooltips, joined nominal suffix headwords, added development metrics, and added authoritative `漢字《かな》` input handling. | `63b5300` |
| 1 | Complete | Successful reader analysis records lexical exposures after scoring; internal merge refreshes do not double-count. | `63b8dad` |
| 2 | Pending | Cover grammar found in current examples, allow an external pattern source, and render recognized grammar in blue. | - |
| 3 | Complete | Double-click actions can split a token at real morpheme boundaries or apply deterministic Top-N segmentation candidates. | Pending item commit |
| 4 | Pending | Remove hard-coded Tauri data paths and use application resource/data directories. | - |
| 5 | Pending | Sanitize MDict HTML and restore an effective CSP. | - |
| 6 | Pending | Add dictionary reading data and reading fallback lookup. | - |
| 7 | Pending | Complete export hash, JLPT, nested context, and user-note fields. | - |
| Lexical boundary | Pending | Prefer a complete dictionary headword for suffixes such as `者` and `署`; otherwise treat the suffix separately. | - |

## Verification Log

- `cargo test -p kotoclip-core pipeline:: -- --nocapture`: passed (6 tests).
- `npm run build`: passed after the baseline frontend changes.
- Item 1: profile test proves the next occurrence receives a lower novelty score.
- Item 3: candidate unit test, frontend production build, and `cargo check -p tauri-app` passed.
