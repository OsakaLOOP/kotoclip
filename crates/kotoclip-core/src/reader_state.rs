use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct ReaderState {
    connection: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordState {
    pub base_form: String,
    pub reading: String,
    pub known: bool,
    pub exposures: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSelection {
    pub book_id: String,
    pub text_version: String,
    pub start: usize,
    pub end: usize,
    pub surface: String,
    pub base_form: String,
    pub reading: String,
    pub note: String,
}

impl ReaderState {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let connection = Connection::open(path)?;
        connection.execute_batch("BEGIN;
            CREATE TABLE IF NOT EXISTS exposure_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                base_form TEXT NOT NULL, reading TEXT,
                pos TEXT, exposure_count INTEGER DEFAULT 0,
                last_seen_at TEXT, is_known BOOLEAN DEFAULT FALSE,
                UNIQUE(base_form, reading));
            CREATE TABLE IF NOT EXISTS reader_selections (
                book_id TEXT NOT NULL, text_version TEXT NOT NULL,
                start INTEGER NOT NULL, end INTEGER NOT NULL,
                surface TEXT NOT NULL, base_form TEXT NOT NULL,
                reading TEXT NOT NULL, note TEXT NOT NULL DEFAULT '',
                PRIMARY KEY(book_id, text_version, start, end));
            PRAGMA user_version = 1;
            COMMIT;")?;
        Ok(Self { connection })
    }

    pub fn word(&self, base_form: &str, reading: &str) -> Result<WordState, rusqlite::Error> {
        self.connection.query_row(
            "SELECT is_known, exposure_count FROM exposure_history WHERE base_form = ?1 AND COALESCE(reading, '') = ?2",
            params![base_form, reading],
            |row| Ok(WordState { base_form: base_form.into(), reading: reading.into(), known: row.get(0)?, exposures: row.get(1)? }),
        ).optional().map(|value| value.unwrap_or(WordState { base_form: base_form.into(), reading: reading.into(), known: false, exposures: 0 }))
    }

    pub fn mark(&self, base_form: &str, reading: &str, known: bool) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "INSERT INTO exposure_history (base_form, reading, is_known, last_seen_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(base_form, reading) DO UPDATE SET is_known = excluded.is_known, last_seen_at = excluded.last_seen_at",
            params![base_form, reading, known],
        )?;
        Ok(())
    }

    pub fn expose(&self, base_form: &str, reading: &str) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "INSERT INTO exposure_history (base_form, reading, exposure_count, last_seen_at)
             VALUES (?1, ?2, 1, datetime('now'))
             ON CONFLICT(base_form, reading) DO UPDATE SET exposure_count = exposure_count + 1, last_seen_at = excluded.last_seen_at",
            params![base_form, reading],
        )?;
        Ok(())
    }

    pub fn selections(&self, book_id: &str, text_version: &str) -> Result<Vec<SavedSelection>, rusqlite::Error> {
        let mut statement = self.connection.prepare(
            "SELECT start, end, surface, base_form, reading, note FROM reader_selections
             WHERE book_id = ?1 AND text_version = ?2 ORDER BY start")?;
        let selections = statement.query_map(params![book_id, text_version], |row| Ok(SavedSelection {
            book_id: book_id.into(), text_version: text_version.into(), start: row.get(0)?, end: row.get(1)?,
            surface: row.get(2)?, base_form: row.get(3)?, reading: row.get(4)?, note: row.get(5)?,
        }))?.collect();
        selections
    }

    pub fn save_selection(&self, selection: &SavedSelection) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "INSERT INTO reader_selections (book_id, text_version, start, end, surface, base_form, reading, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(book_id, text_version, start, end) DO UPDATE SET note = excluded.note",
            params![selection.book_id, selection.text_version, selection.start, selection.end,
                selection.surface, selection.base_form, selection.reading, selection.note],
        )?;
        Ok(())
    }

    pub fn delete_selection(&self, book_id: &str, text_version: &str, start: usize, end: usize) -> Result<(), rusqlite::Error> {
        self.connection.execute("DELETE FROM reader_selections WHERE book_id = ?1 AND text_version = ?2 AND start = ?3 AND end = ?4",
            params![book_id, text_version, start, end])?;
        Ok(())
    }

    pub fn clear_selections(&self, book_id: &str, text_version: &str) -> Result<(), rusqlite::Error> {
        self.connection.execute("DELETE FROM reader_selections WHERE book_id = ?1 AND text_version = ?2", params![book_id, text_version])?;
        Ok(())
    }
}
