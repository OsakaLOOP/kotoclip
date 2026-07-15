"""逐记录验证旧 schema v3 与新 schema v4 的词条、别名和查询键一致性。"""

from __future__ import annotations

import argparse
import hashlib
import itertools
import sqlite3
import zlib
from pathlib import Path


def update_hash(digest: hashlib._Hash, *values: object) -> None:
    for value in values:
        encoded = str(value).encode("utf-8")
        digest.update(len(encoded).to_bytes(4, "little"))
        digest.update(encoded)


def compare_rows(label: str, old_rows, new_rows) -> tuple[int, str, str]:
    old_hash = hashlib.sha256()
    new_hash = hashlib.sha256()
    count = 0
    mismatches: list[tuple[object, object]] = []
    for old_row, new_row in itertools.zip_longest(old_rows, new_rows):
        count += 1
        if old_row is not None:
            update_hash(old_hash, *old_row)
        if new_row is not None:
            update_hash(new_hash, *new_row)
        if old_row != new_row and len(mismatches) < 5:
            mismatches.append((old_row, new_row))
    if mismatches:
        raise AssertionError(f"{label} 不一致，样例：{mismatches}")
    return count, old_hash.hexdigest(), new_hash.hexdigest()


def canonical_rows(connection: sqlite3.Connection):
    yield from connection.execute(
        "SELECT id, headword, definition FROM entries "
        "WHERE definition NOT LIKE '@@@LINK=%' ORDER BY id"
    )


def verify(old_path: Path, new_path: Path) -> None:
    old = sqlite3.connect(old_path)
    new = sqlite3.connect(new_path)
    old_canonical = list(canonical_rows(old))
    old_to_new_id = {old_id: index for index, (old_id, _, _) in enumerate(old_canonical, 1)}

    blocks = {
        block_id: zlib.decompress(data)
        for block_id, data in new.execute("SELECT id, data FROM definition_blocks")
    }

    def old_entries():
        for _, headword, definition in old_canonical:
            yield headword, definition

    def new_entries():
        for headword, block_id, offset, length in new.execute(
            "SELECT headword, definition_block_id, definition_offset, definition_length "
            "FROM entries ORDER BY id"
        ):
            definition = blocks[block_id][offset : offset + length].decode("utf-8")
            yield headword, definition

    reports = {
        "entries": compare_rows("规范词条", old_entries(), new_entries()),
        "aliases": compare_rows(
            "别名关系",
            old.execute(
                "SELECT headword, substr(definition, 9) FROM entries "
                "WHERE definition LIKE '@@@LINK=%' ORDER BY headword, substr(definition, 9)"
            ),
            new.execute("SELECT alias, target FROM aliases ORDER BY alias, target"),
        ),
    }

    def old_keys(table: str, normalized: str, display: str):
        query = (
            f"SELECT entry_id, {normalized}, {display}, is_primary FROM {table} "
            f"ORDER BY entry_id, {normalized}"
        )
        for entry_id, normalized_value, display_value, is_primary in old.execute(query):
            mapped = old_to_new_id.get(entry_id)
            if mapped is not None:
                yield (
                    mapped,
                    normalized_value,
                    None if display_value == normalized_value else display_value,
                    bool(is_primary),
                )

    def new_keys(kind: int):
        for entry_id, normalized_value, display_value, rank in new.execute(
            "SELECT entry_id, normalized_value, display_value, rank FROM entry_keys "
            "WHERE kind = ? ORDER BY entry_id, normalized_value",
            (kind,),
        ):
            yield entry_id, normalized_value, display_value, rank == 0

    reports["forms"] = compare_rows(
        "表记键",
        old_keys("entry_forms", "normalized_form", "form"),
        new_keys(0),
    )
    reports["readings"] = compare_rows(
        "读音键",
        old_keys("entry_readings", "normalized_reading", "reading"),
        new_keys(1),
    )

    def old_alias_keys(table: str, normalized: str):
        yield from old.execute(
            f"SELECT e.headword, substr(e.definition, 9), ?1, k.{normalized} "
            f"FROM {table} k JOIN entries e ON e.id = k.entry_id "
            "WHERE e.definition LIKE '@@@LINK=%' "
            f"ORDER BY e.headword, substr(e.definition, 9), k.{normalized}",
            (0 if table == "entry_forms" else 1,),
        )

    reports["alias_forms"] = compare_rows(
        "别名表记键",
        old_alias_keys("entry_forms", "normalized_form"),
        new.execute(
            "SELECT alias, target, kind, normalized_value FROM alias_keys "
            "WHERE kind = 0 ORDER BY alias, target, normalized_value"
        ),
    )
    reports["alias_readings"] = compare_rows(
        "别名读音键",
        old_alias_keys("entry_readings", "normalized_reading"),
        new.execute(
            "SELECT alias, target, kind, normalized_value FROM alias_keys "
            "WHERE kind = 1 ORDER BY alias, target, normalized_value"
        ),
    )

    for label, (count, old_hash, new_hash) in reports.items():
        print(f"{label}: rows={count} sha256={old_hash} matched={old_hash == new_hash}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("old", type=Path)
    parser.add_argument("new", type=Path)
    args = parser.parse_args()
    verify(args.old, args.new)


if __name__ == "__main__":
    main()
