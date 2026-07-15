"""Kotoclip 词典 schema v4 的词头解析与规范化工具。"""

from __future__ import annotations

import unicodedata
from collections.abc import Iterable


SCHEMA_VERSION = 4
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


def is_kana_form(value: str) -> bool:
    """与旧 schema 保持一致：含假名且不含汉字的词头视为读音。"""
    has_kanji = any(
        "\u3400" <= char <= "\u9fff" or "\uf900" <= char <= "\ufaff"
        for char in value
    )
    has_kana = any("ぁ" <= char <= "ヿ" for char in value)
    return has_kana and not has_kanji


def parse_headword(raw_headword: str) -> tuple[list[str], list[str]]:
    """解析 ``かな【表記・別表記】``，返回表记与读音。"""
    raw = unicodedata.normalize("NFKC", raw_headword).strip()
    if not raw:
        return [], []

    forms: list[str] = []
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
                forms.append(cleaned)
    else:
        cleaned = raw.translate(EDITORIAL_MARKS).strip()
        if cleaned:
            forms.append(cleaned)
            if is_kana_form(cleaned):
                readings.append(cleaned)
    return forms, readings


def keys_for_entry(
    raw_headword: str,
    structured_reading: str | None,
) -> tuple[list[tuple[str, str]], list[tuple[str, str]]]:
    """返回按优先级排序的 ``(显示值, 规范值)`` 表记与读音。"""
    forms, parsed_readings = parse_headword(raw_headword)
    form_rows: list[tuple[str, str]] = []
    reading_rows: list[tuple[str, str]] = []
    seen_forms: set[str] = set()
    seen_readings: set[str] = set()

    for form in forms:
        normalized = normalize_form(form)
        if normalized and normalized not in seen_forms:
            form_rows.append((form, normalized))
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
