"""Kotoclip 词典 schema v3 与大辞林词头解析工具。"""

from __future__ import annotations

import sqlite3
import unicodedata
from collections.abc import Iterable


SCHEMA_VERSION = 3
SEPARATORS = str.maketrans("", "", " \t\r\n・･-‐‑‒–—―")
EDITORIAL_MARKS = str.maketrans("", "", "▽▼△▲×")


def normalize_form(value: str) -> str:
    """统一全半角，并移除词典用于分段的字符。"""
    normalized = unicodedata.normalize("NFKC", value).replace("繋", "繫")
    return normalized.translate(SEPARATORS).strip()


def normalize_reading(value: str) -> str:
    """将平假名读音规范化为片假名，保留长音及非假名字符。"""
    normalized = normalize_form(value)
    return "".join(
        chr(ord(char) + 0x60) if "ぁ" <= char <= "ゖ" else char
        for char in normalized
    )


def classify_form(value: str) -> str:
    has_kanji = any("\u3400" <= char <= "\u9fff" or "\uf900" <= char <= "\ufaff" for char in value)
    has_kana = any("ぁ" <= char <= "ヿ" for char in value)
    if has_kanji and has_kana:
        return "mixed"
    if has_kanji:
        return "kanji"
    if has_kana:
        return "kana"
    return "other"


def parse_headword(raw_headword: str) -> tuple[list[tuple[str, str]], list[str]]:
    """解析 ``かな【表記・別表記】``，返回表记与读音；原始键由 entries 保留。"""
    raw = unicodedata.normalize("NFKC", raw_headword).strip()
    if not raw:
        return [], []

    forms: list[tuple[str, str]] = []
    readings: list[str] = []
    bracket_start = raw.find("【")
    bracket_end = raw.rfind("】")
    if bracket_start >= 0 and bracket_end > bracket_start:
        reading = raw[:bracket_start].strip()
        if reading:
            readings.append(reading)
        for spelling in raw[bracket_start + 1 : bracket_end].split("・"):
            cleaned = spelling.translate(EDITORIAL_MARKS).strip()
            if cleaned:
                forms.append((cleaned, classify_form(cleaned)))
    else:
        cleaned = raw.translate(EDITORIAL_MARKS).strip()
        if cleaned:
            forms.append((cleaned, classify_form(cleaned)))
            if classify_form(cleaned) == "kana":
                readings.append(cleaned)
    return forms, readings


def ensure_schema(connection: sqlite3.Connection) -> None:
    connection.execute(
        """
        CREATE TABLE IF NOT EXISTS entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            headword TEXT NOT NULL,
            reading TEXT,
            definition TEXT NOT NULL,
            dict_name TEXT NOT NULL
        )
        """
    )
    columns = {row[1] for row in connection.execute("PRAGMA table_info(entries)")}
    if "reading" not in columns:
        connection.execute("ALTER TABLE entries ADD COLUMN reading TEXT")

    connection.executescript(
        """
        CREATE INDEX IF NOT EXISTS idx_entries_headword ON entries(headword);
        CREATE INDEX IF NOT EXISTS idx_entries_reading ON entries(reading);
        CREATE TABLE IF NOT EXISTS metadata (
            schema_version INTEGER NOT NULL,
            source_name TEXT NOT NULL,
            imported_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS entry_forms (
            entry_id INTEGER NOT NULL,
            form TEXT NOT NULL,
            normalized_form TEXT NOT NULL,
            form_type TEXT NOT NULL CHECK(form_type IN ('kanji', 'kana', 'mixed', 'other')),
            is_primary INTEGER NOT NULL DEFAULT 0 CHECK(is_primary IN (0, 1)),
            PRIMARY KEY(entry_id, normalized_form),
            FOREIGN KEY(entry_id) REFERENCES entries(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS entry_readings (
            entry_id INTEGER NOT NULL,
            reading TEXT NOT NULL,
            normalized_reading TEXT NOT NULL,
            is_primary INTEGER NOT NULL DEFAULT 0 CHECK(is_primary IN (0, 1)),
            PRIMARY KEY(entry_id, normalized_reading),
            FOREIGN KEY(entry_id) REFERENCES entries(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_entry_forms_normalized
            ON entry_forms(normalized_form, form_type);
        CREATE INDEX IF NOT EXISTS idx_entry_readings_normalized
            ON entry_readings(normalized_reading);
        """
    )


def keys_for_entry(raw_headword: str, structured_reading: str | None) -> tuple[list[tuple[str, str, str]], list[tuple[str, str]]]:
    forms, parsed_readings = parse_headword(raw_headword)
    form_rows: list[tuple[str, str, str]] = []
    reading_rows: list[tuple[str, str]] = []
    seen_forms: set[str] = set()
    seen_readings: set[str] = set()

    for form, form_type in forms:
        normalized = normalize_form(form)
        if normalized and normalized not in seen_forms:
            form_rows.append((form, normalized, form_type))
            seen_forms.add(normalized)

    candidates: Iterable[str] = parsed_readings
    if structured_reading:
        candidates = [structured_reading, *parsed_readings]
    for reading in candidates:
        normalized = normalize_reading(reading)
        if normalized and normalized not in seen_readings:
            reading_rows.append((reading, normalized))
            seen_readings.add(normalized)
    return form_rows, reading_rows


def rebuild_search_tables(connection: sqlite3.Connection, batch_size: int = 10_000) -> tuple[int, int, int]:
    """从 entries 的原始词头重建结构化表记与读音索引。"""
    ensure_schema(connection)
    connection.execute("DELETE FROM entry_forms")
    connection.execute("DELETE FROM entry_readings")

    scanned = 0
    forms_written = 0
    readings_written = 0
    form_batch: list[tuple[int, str, str, str, int]] = []
    reading_batch: list[tuple[int, str, str, int]] = []
    for entry_id, raw_headword, structured_reading in connection.execute(
        "SELECT id, headword, reading FROM entries ORDER BY id"
    ):
        scanned += 1
        forms, readings = keys_for_entry(raw_headword, structured_reading)
        form_batch.extend(
            (entry_id, form, normalized, form_type, int(index == 0))
            for index, (form, normalized, form_type) in enumerate(forms)
        )
        reading_batch.extend(
            (entry_id, reading, normalized, int(index == 0))
            for index, (reading, normalized) in enumerate(readings)
        )
        if len(form_batch) + len(reading_batch) >= batch_size:
            forms_written += _flush_forms(connection, form_batch)
            readings_written += _flush_readings(connection, reading_batch)
            connection.commit()

    forms_written += _flush_forms(connection, form_batch)
    readings_written += _flush_readings(connection, reading_batch)
    connection.commit()
    return scanned, forms_written, readings_written


def _flush_forms(connection: sqlite3.Connection, rows: list[tuple[int, str, str, str, int]]) -> int:
    count = len(rows)
    if rows:
        connection.executemany(
            "INSERT OR IGNORE INTO entry_forms(entry_id, form, normalized_form, form_type, is_primary) VALUES (?, ?, ?, ?, ?)",
            rows,
        )
        rows.clear()
    return count


def _flush_readings(connection: sqlite3.Connection, rows: list[tuple[int, str, str, int]]) -> int:
    count = len(rows)
    if rows:
        connection.executemany(
            "INSERT OR IGNORE INTO entry_readings(entry_id, reading, normalized_reading, is_primary) VALUES (?, ?, ?, ?)",
            rows,
        )
        rows.clear()
    return count
