"""生成 Kotoclip 可分发词典源包。"""

from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import struct
import zlib
from collections.abc import Iterable, Iterator
from pathlib import Path

from dictionary_schema import SCHEMA_VERSION, keys_for_entry, normalize_form


MAGIC = b"KDICT\x00\x01\x00"
FORMAT_VERSION = 1
DEFAULT_BLOCK_SIZE = 1024 * 1024
CANONICAL_ENTRY = 0
ALIAS_ENTRY = 1


def _pack_u16(value: int) -> bytes:
    return struct.pack("<H", value)


def _pack_u32(value: int) -> bytes:
    return struct.pack("<I", value)


def _pack_u64(value: int) -> bytes:
    return struct.pack("<Q", value)


def _pack_text(value: str) -> bytes:
    encoded = value.encode("utf-8")
    return _pack_u32(len(encoded)) + encoded


def _pack_optional_text(value: str | None) -> bytes:
    if value is None:
        return _pack_u32(0xFFFFFFFF)
    return _pack_text(value)


def _source_name_from_sqlite(connection: sqlite3.Connection, fallback: str) -> str:
    try:
        row = connection.execute(
            "SELECT source_name FROM metadata ORDER BY rowid DESC LIMIT 1"
        ).fetchone()
    except sqlite3.Error:
        row = None
    return str(row[0]) if row and row[0] else fallback


def iter_sqlite_entries(path: Path) -> tuple[str, Iterator[tuple[str, str | None, str]]]:
    connection = sqlite3.connect(path)
    columns = {row[1] for row in connection.execute("PRAGMA table_info(entries)")}
    required = {"headword", "definition"}
    if not required.issubset(columns):
        connection.close()
        raise ValueError(f"不支持的 SQLite 词典：{path}")
    reading_sql = "reading" if "reading" in columns else "NULL"
    source_name = _source_name_from_sqlite(connection, path.stem)

    def rows() -> Iterator[tuple[str, str | None, str]]:
        try:
            for headword, reading, definition in connection.execute(
                f"SELECT headword, {reading_sql}, definition FROM entries ORDER BY id"
            ):
                yield str(headword), str(reading) if reading else None, str(definition)
        finally:
            connection.close()

    return source_name, rows()


def iter_mdx_entries(path: Path, encoding_override: str | None = None) -> Iterator[tuple[str, None, str]]:
    try:
        from mdict_utils.base.readmdict import MDX
    except ImportError:
        try:
            from readmdict import MDX
        except ImportError as error:
            raise RuntimeError("读取 MDX 需要安装 readmdict：pip install readmdict") from error

    kwargs = {}
    if encoding_override:
        kwargs["encoding"] = encoding_override

    for raw_headword, raw_definition in MDX(str(path), **kwargs).items():
        headword = (
            raw_headword.decode("utf-8", errors="ignore").strip()
            if isinstance(raw_headword, bytes)
            else raw_headword.strip()
        )
        definition = (
            raw_definition.decode("utf-8", errors="ignore").strip()
            if isinstance(raw_definition, bytes)
            else raw_definition.strip()
        )
        if headword and definition:
            yield headword, None, definition


def iter_txt_entries(path: Path) -> Iterator[tuple[str, str | None, str]]:
    state = "head"
    headword = ""
    reading: str | None = None
    definition_lines: list[str] = []
    with path.open("r", encoding="utf-8", errors="ignore") as source:
        for line in source:
            value = line.rstrip("\r\n")
            if state == "head":
                parts = value.split("\t", 1)
                headword = parts[0].strip()
                reading = parts[1].strip() if len(parts) == 2 and parts[1].strip() else None
                if headword:
                    definition_lines = []
                    state = "body"
            elif value == "</>":
                definition = "\n".join(definition_lines).strip()
                if definition:
                    yield headword, reading, definition
                state = "head"
                headword = ""
                reading = None
            else:
                definition_lines.append(value)


def source_entries(
    path: Path,
    source_name: str | None,
) -> tuple[str, Iterable[tuple[str, str | None, str]]]:
    suffix = path.suffix.lower()
    default_name = path.stem
    if Path(default_name).suffix.lower() == ".mdx":
        default_name = Path(default_name).stem
    if suffix == ".mdx":
        return source_name or default_name, iter_mdx_entries(path)
    if suffix in {".db", ".sqlite"}:
        detected_name, entries = iter_sqlite_entries(path)
        return source_name or detected_name, entries
    if suffix in {".txt", ".tsv"}:
        return source_name or default_name, iter_txt_entries(path)
    raise ValueError(f"不支持的词典来源：{path}")


def build_bundle(
    source_path: Path,
    output_path: Path,
    source_name: str | None = None,
    block_size: int = DEFAULT_BLOCK_SIZE,
) -> dict[str, object]:
    """把原始词典转换为可分发、可重建 SQLite 的紧凑源包。"""
    source_path = source_path.resolve()
    if not source_path.is_file():
        raise FileNotFoundError(source_path)
    detected_name, entries = source_entries(source_path, source_name)
    return build_bundle_from_entries(entries, output_path, detected_name, block_size)


def build_bundle_from_entries(
    entries: Iterable[tuple[str, str | None, str]],
    output_path: Path,
    source_name: str,
    block_size: int = DEFAULT_BLOCK_SIZE,
) -> dict[str, object]:
    """从已解析词条生成源包，供 starter 与测试复用。"""
    output_path = output_path.resolve()
    if block_size < 64 * 1024:
        raise ValueError("definition block 至少为 64 KiB")
    metadata = bytearray()
    definition_block = bytearray()
    compressed_blocks: list[tuple[int, bytes]] = []
    canonical_count = 0
    alias_count = 0
    source_hash = hashlib.sha256()

    def flush_block() -> None:
        if not definition_block:
            return
        raw = bytes(definition_block)
        compressed_blocks.append((len(raw), zlib.compress(raw, 9)))
        definition_block.clear()

    for raw_headword, structured_reading, raw_definition in entries:
        headword = raw_headword.strip()
        definition = raw_definition.strip()
        if not headword or not definition:
            continue
        headword_bytes = headword.encode("utf-8")
        definition_bytes = definition.encode("utf-8")
        source_hash.update(_pack_u32(len(headword_bytes)))
        source_hash.update(headword_bytes)
        reading_bytes = (structured_reading or "").encode("utf-8")
        source_hash.update(_pack_u32(len(reading_bytes)))
        source_hash.update(reading_bytes)
        source_hash.update(_pack_u32(len(definition_bytes)))
        source_hash.update(definition_bytes)

        if definition.startswith("@@@LINK="):
            target = definition[8:].strip()
            if not target:
                continue
            normalized_alias = normalize_form(headword)
            metadata.append(ALIAS_ENTRY)
            metadata.extend(_pack_text(headword))
            metadata.extend(
                _pack_optional_text(normalized_alias if normalized_alias != headword else None)
            )
            metadata.extend(_pack_text(target))
            forms, readings = keys_for_entry(headword, structured_reading)
            metadata.extend(_pack_u16(len(forms)))
            for _, normalized in forms:
                metadata.extend(_pack_text(normalized))
            metadata.extend(_pack_u16(len(readings)))
            for _, normalized in readings:
                metadata.extend(_pack_text(normalized))
            alias_count += 1
            continue

        if definition_block and len(definition_block) + len(definition_bytes) > block_size:
            flush_block()
        block_id = len(compressed_blocks) + 1
        offset = len(definition_block)
        definition_block.extend(definition_bytes)
        forms, readings = keys_for_entry(headword, structured_reading)

        metadata.append(CANONICAL_ENTRY)
        metadata.extend(_pack_text(headword))
        metadata.extend(_pack_u32(block_id))
        metadata.extend(_pack_u32(offset))
        metadata.extend(_pack_u32(len(definition_bytes)))
        metadata.extend(_pack_u16(len(forms)))
        for rank, (display, normalized) in enumerate(forms):
            metadata.extend(_pack_text(normalized))
            metadata.extend(_pack_optional_text(display if display != normalized else None))
            metadata.extend(_pack_u16(rank))
        metadata.extend(_pack_u16(len(readings)))
        for rank, (display, normalized) in enumerate(readings):
            metadata.extend(_pack_text(normalized))
            metadata.extend(_pack_optional_text(display if display != normalized else None))
            metadata.extend(_pack_u16(rank))
        canonical_count += 1

    flush_block()
    metadata_raw = bytes(metadata)
    metadata_compressed = zlib.compress(metadata_raw, 9)
    bundle_hash = hashlib.sha256()
    bundle_hash.update(MAGIC)
    bundle_hash.update(_pack_u32(SCHEMA_VERSION))
    bundle_hash.update(source_hash.digest())
    bundle_hash.update(metadata_raw)
    header = {
        "format_version": FORMAT_VERSION,
        "schema_version": SCHEMA_VERSION,
        "source_name": source_name,
        "bundle_id": bundle_hash.hexdigest(),
        "canonical_count": canonical_count,
        "alias_count": alias_count,
        "definition_block_count": len(compressed_blocks),
        "definition_block_size": block_size,
        "metadata_uncompressed_size": len(metadata_raw),
    }
    header_bytes = json.dumps(
        header,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    temporary = output_path.with_suffix(output_path.suffix + ".building")
    with temporary.open("wb") as output:
        output.write(MAGIC)
        output.write(_pack_u32(len(header_bytes)))
        output.write(header_bytes)
        output.write(_pack_u64(len(metadata_raw)))
        output.write(_pack_u64(len(metadata_compressed)))
        output.write(metadata_compressed)
        output.write(_pack_u32(len(compressed_blocks)))
        for uncompressed_size, compressed in compressed_blocks:
            output.write(_pack_u32(uncompressed_size))
            output.write(_pack_u32(len(compressed)))
            output.write(compressed)
    os.replace(temporary, output_path)
    return {**header, "output_bytes": output_path.stat().st_size}
