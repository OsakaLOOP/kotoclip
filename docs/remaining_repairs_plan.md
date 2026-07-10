# Kotoclip Remaining Repairs Plan

Updated: 2026-07-10

## Scope

This document is the implementation plan for work intentionally deferred after commits
`63b8dad` (exposure tracking) and `7be30e8` (split and segmentation candidates).
No items below are implemented by this document.

Recommended order:

1. Portable application paths
2. Dictionary reading schema and lookup fallback
3. Dictionary-aware lexical boundaries
4. MDict HTML sanitization and CSP
5. Export contract completion
6. External grammar patterns and blue grammar rendering

The ordering matters. Lexical-boundary decisions require a reliable dictionary API, and
grammar coloring requires precise matched ranges rather than the current bunsetsu-level tag.

## A. Portable Tauri Paths

### Objective

Remove `D:\PROJ\GIT\kotoclip` from runtime code and make development, installed desktop,
and test environments resolve resources consistently.

### Design

- Add `src-tauri/src/paths.rs` with an `AppPaths` structure:
  - `system_dictionary`: read-only packaged `ipadic/system.dic`.
  - `dictionary_dir`: writable `<app_data_dir>/dicts`.
  - `profile_db`: writable `<app_data_dir>/user_profile.db`.
- Resolve paths inside `tauri::Builder::setup`, where `AppHandle::path()` is available.
- Allow `KOTOCLIP_DATA_DIR` only as an explicit development/test override.
- Copy a packaged starter dictionary database only when the writable destination is absent.
  Never overwrite user dictionaries or profiles.
- Add required bundle resources to `src-tauri/tauri.conf.json` and rename remaining scaffold
  metadata (`productName`, title, description) to Kotoclip.
- Replace absolute test paths with a shared test helper that reads `KOTOCLIP_TEST_IPADIC`
  and otherwise skips resource-dependent tests with a clear reason.

### Files

- `src-tauri/src/lib.rs`
- `src-tauri/src/paths.rs` (new)
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- resource-dependent tests under `crates/kotoclip-core/src/pipeline/`

### Verification

- Run with the repository located outside `D:\PROJ\GIT`.
- Run with `KOTOCLIP_DATA_DIR` pointing to a temporary directory.
- Confirm first-run directory creation and second-run profile preservation.
- Build an installer and confirm `system.dic` is resolved from packaged resources.

## B. Dictionary Reading Schema and Fallback

### Objective

Support exact headword lookup, reading lookup, and fuzzy fallback in that order.

### Schema

```sql
ALTER TABLE entries ADD COLUMN reading TEXT;
CREATE INDEX IF NOT EXISTS idx_entries_reading ON entries(reading);
```

The importer must create new databases with `reading TEXT` from the start. Normalize both
stored and queried readings to katakana and Unicode NFC. Do not mutate existing read-only
databases at application startup.

### Import and Migration

- Update `scripts/mdx_to_sqlite.py` and `scripts/txt_to_sqlite.py` to extract reading metadata
  when the source format exposes it.
- Add `scripts/migrate_dictionary_schema.py` for an explicit, backed-up migration.
- When a source lacks structured reading metadata, leave `reading` null rather than guessing
  from definition prose.
- Version imported databases with a `metadata(schema_version, source_name, imported_at)` table.

### API

- Change lookup input to `{ headword, reading? }` throughout Rust, Tauri, and TypeScript.
- Query order:
  1. `entries.headword = ?`
  2. `entries.reading = ?` when reading is present
  3. escaped FTS query with a strict result cap
- Return match provenance (`headword`, `reading`, or `fuzzy`) so UI can label fallback results.
- Cache exact-existence checks separately from full definition results.

### Verification

- Exact headword wins over a reading collision.
- Inflected input resolves through its authoritative ruby/IPADIC reading.
- Missing `reading` columns produce a clear compatibility error with migration instructions.
- FTS special characters cannot break the query.

## C. Dictionary-Aware Lexical Boundaries (`者`, `署`, etc.)

### Objective

Treat a nominal suffix as part of the lexical head only when the complete candidate exists in
the dictionary. Otherwise keep the root as the lookup head and annotate the suffix as grammar.

### Design

- Stop unconditionally joining every `名詞,接尾` in `pipeline/bunsetsu.rs`.
- Preserve a lexical candidate consisting of the root plus consecutive nominal suffixes.
- Add `DictionaryEngine::contains_exact(&str) -> bool` using an indexed `SELECT EXISTS` query.
- In `Engine`, run a post-chunk lexical-boundary resolver before profile scoring:
  - If the complete candidate exists, set head surface/base/reading to the complete word.
  - Otherwise retain the root head and create a suffix grammar tag covering only the suffix.
- Cache existence decisions for the process lifetime; dictionary files are immutable while open.
- Ruby remains authoritative for reading regardless of the boundary decision.

### Model Change

Add `morpheme_range` and `char_range` to `GrammarTag`. This is required so only `者` or `署`
is rendered as grammar instead of coloring the entire bunsetsu.

### Tests

- A temporary SQLite dictionary containing `警察署` makes all of `警察署` the headword.
- Without `警察署`, `警察` stays the head and `署` receives a suffix grammar tag.
- Repeat for `はぐれ者` and a dictionary containing/not containing the complete word.
- Confirm the definition lookup uses the selected headword.

## D. MDict HTML Sanitization and CSP

### Objective

Prevent imported dictionary HTML from executing code or breaking application layout.

### Backend Sanitization

- Add the Rust `ammonia` crate and sanitize `definition_html` before it crosses IPC.
- Allow only dictionary-safe structural tags such as `p`, `div`, `span`, `br`, `ruby`, `rt`,
  `rp`, `b`, `strong`, `i`, `em`, `ul`, `ol`, `li`, `dl`, `dt`, `dd`, and `a`.
- Remove `script`, `style`, iframe/object/embed elements, all `on*` attributes, inline `style`,
  and unsafe URL schemes.
- Add a maximum definition byte size and a visible truncation marker for malformed entries.
- Keep raw HTML in SQLite so sanitation rules can evolve without reimporting dictionaries.

### Frontend Containment

- Render only the sanitized field through `v-html`.
- Apply `contain: content`, `max-width: 100%`, wrapping, and media size limits to dictionary HTML.
- Do not let dictionary markup define fixed positioning or viewport-sized elements.

### CSP

Replace `"csp": null` with a Tauri v2 policy restricted to the application, IPC transport,
data images where required, and local assets. Validate the exact IPC directives against the
Tauri version in `Cargo.lock` before committing.

### Security Tests

- Strip scripts, event handlers, `javascript:` links, fixed positioning, and injected styles.
- Preserve Japanese ruby and safe dictionary structure.
- Confirm tooltip and modal content cannot escape their bounds.

## E. Export Contract Completion

### Objective

Match the original versioned export shape and preserve enough context for Anki and notes.

### Target Shape

```json
{
  "version": "1.0",
  "exported_at": "RFC3339",
  "source_text_hash": "sha256:...",
  "entries": [
    {
      "surface": "...",
      "base_form": "...",
      "reading": "...",
      "pos": "...",
      "grammar_tags": ["..."],
      "jlpt_levels": [3],
      "context": {
        "sentence": "...",
        "highlight_range": [0, 1]
      },
      "definitions": [],
      "user_note": ""
    }
  ]
}
```

### Design

- Add `sha2`; hash the canonical text after `漢字《かな》` markup removal so offsets and hash
  describe the same analysis object.
- Move sentence extraction to a tested helper using Japanese and ASCII terminal punctuation.
- Derive unique sorted JLPT levels from grammar tags.
- Add editable per-entry notes in `ExportPanel.vue`, keyed by paragraph/token identity for the
  current session.
- Send source text and structured entries to Rust; Rust owns schema versioning, hashing,
  timestamping, and serialization.
- Deduplicate identical `(base_form, reading)` entries while preserving all source contexts or
  explicitly document single-context behavior.

### Verification

- JSON schema snapshot test.
- Hash is stable for identical canonical text and changes with source text.
- UTF-16 JavaScript offsets are never mixed with Rust character offsets.
- Notes, JLPT levels, sanitized definitions, and sentence highlights round-trip.

## F. External Grammar Patterns and Blue Rendering

### Objective

Cover grammar present in current examples while making the rule set replaceable by an external
source. A manually maintained 100-rule seed set is explicitly not required for this phase.

### Initial Example Coverage

- `〜ている` and colloquial `〜てんだ`
- `〜てくる`
- `〜てやる`
- `〜つもりだ`
- `〜ておく`
- `〜ながら（も）`
- passive `〜れる／られる`
- negative colloquial `〜ん` where constraints distinguish it from explanatory `の`

### Pattern Source

- Move built-in pattern construction to versioned JSON with strict serde validation.
- Load user patterns from the application data directory and fall back to bundled patterns.
- Reject duplicate IDs, invalid POS alphabets, out-of-range constraint indexes, and empty names.
- Compile Aho-Corasick once at engine startup and report invalid external files without crashing.
- Keep the existing overlapping-match behavior and add deterministic conflict resolution for
  nested patterns (longest span, then configured priority).

### Precise Blue Rendering

- Extend `GrammarTag` with matched morpheme/character ranges.
- In `BunsetsuCapsule.vue`, assign a grammar class only to matched morphemes.
- Define a dedicated accessible blue palette independent of novelty red/orange.
- Grammar badges and matched grammar text use blue; lexical heads retain novelty coloring.
- E-ink mode uses underline plus a compact grammar badge because color is unavailable.

### Verification

- Regression tests use both user-provided example paragraphs.
- External and bundled files produce the same model contract.
- Invalid pattern files fall back safely and expose a diagnostic.
- Visual verification confirms only recognized grammar spans are blue.

## G. True Tokenizer N-Best (Optional Follow-Up)

Vibrato 0.5.2 exposes only the single Viterbi result through its public `Worker` API. The current
Top-N UI therefore ranks deterministic segmentations made from confirmed morpheme boundaries.
If true lattice N-best is required later, choose one of these explicitly:

1. Upgrade to a tokenizer version with a supported N-best API.
2. Replace the tokenizer with a library that exposes lattice alternatives and costs.
3. Maintain a small Vibrato fork exposing a stable k-shortest-path result.

Do not access Vibrato private lattice fields through copied internal code without owning the fork.
Whichever path is chosen must return candidate scores and preserve authoritative ruby ranges.

## Commit Strategy

Each section above should be one focused commit, except dictionary schema plus lexical boundary,
which may use two commits if migration tooling is independently reviewable. For every commit:

1. Update `docs/repair_report.md` with status and the previous commit hash.
2. Run focused Rust/frontend tests for the changed contract.
3. Review `git diff --check` and `git diff` before commit.
4. Before final submission, run the repository-wide pre-commit hook and both required builds.
