# Kotoclip Repair Report

Updated: 2026-07-10

This file tracks the requested repairs. Each numbered item is committed separately.

| Item | Status | Summary | Commit |
| --- | --- | --- | --- |
| Baseline | Complete | Restored first analysis rendering, fixed overlapping grammar matches, measured virtual rows, constrained tooltips, joined nominal suffix headwords, added development metrics, and added authoritative `漢字《かな》` input handling. | `63b5300` |
| 1 | Complete | Successful reader analysis records lexical exposures after scoring; internal merge refreshes do not double-count. | `63b8dad` |
| 2 | Complete | Added validated external grammar loading, current-example seed rules, exact morpheme/character ranges, accessible blue grammar rendering, and E-ink underline fallback. | Working tree |
| 3 | Complete | Double-click actions can split a token at real morpheme boundaries or apply deterministic Top-N segmentation candidates. | `7be30e8` |
| 4 | Complete | Added portable resource/app-data path resolution, explicit `KOTOCLIP_DATA_DIR` override, starter database preservation, packaged resources, and Kotoclip metadata. | Working tree |
| 5 | Complete | Sanitized dictionary HTML at the IPC boundary with size limits and restored restrictive Tauri CSP plus frontend containment styling. | Working tree |
| 6 | Complete | Added reading schema/index, importer support, explicit backup migration, NFC/NFKC katakana lookup normalization, provenance, and strict fallback order. | Working tree |
| 7 | Complete | Added canonical source hash, RFC3339 timestamp, nested context, sorted JLPT levels, and editable notes to the export contract. | Working tree |
| Lexical boundary | Complete | Dictionary-aware suffix resolution now retains complete headwords only when exact entries exist and otherwise emits range-scoped suffix grammar tags. | Working tree |

## Verification Log

- `cargo test -p kotoclip-core pipeline:: -- --nocapture`: passed (6 tests).
- `npm run build`: passed after the baseline frontend changes.
- Item 1: profile test proves the next occurrence receives a lower novelty score.
- Item 3: candidate unit test, frontend production build, and `cargo check -p tauri-app` passed.
