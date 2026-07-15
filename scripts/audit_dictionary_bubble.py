"""审计悬浮词典所依赖的真实词典形态与第一话代表查询。"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import zlib
from collections import Counter
from pathlib import Path


def chapter_text(path: Path, heading: str) -> str:
    text = path.read_text(encoding="utf-8")
    start = text.index(heading)
    rest = text[start + len(heading) :]
    end = re.search(r"^##\s+", rest, re.MULTILINE)
    return rest[: end.start() if end else None]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dict", dest="dictionary", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--chapter", default="## 第一話　冷やし神")
    args = parser.parse_args()

    chapter = chapter_text(args.source, args.chapter)
    required_source_terms = ["七日", "冷やし神", "いる", "繋", "警察署"]
    missing_source = [term for term in required_source_terms if term not in chapter]

    connection = sqlite3.connect(args.dictionary)
    entry_count = connection.execute("SELECT count(*) FROM entries").fetchone()[0]
    alias_count = connection.execute("SELECT count(*) FROM aliases").fetchone()[0]
    blocks = {
        block_id: zlib.decompress(data)
        for block_id, data in connection.execute(
            "SELECT id, data FROM definition_blocks"
        )
    }

    def read_definition(row: tuple[int, int, int] | None) -> str:
        if row is None:
            return ""
        block_id, offset, length = row
        return blocks[block_id][offset : offset + length].decode("utf-8")

    checks: dict[str, bool] = {}
    for query in ["ボリューム", "ひやし", "七日"]:
        checks[f"redirect:{query}"] = bool(
            connection.execute(
                "SELECT 1 FROM aliases WHERE alias = ? LIMIT 1",
                (query,),
            ).fetchone()
        )
    for query in ["いる", "ある", "かみ"]:
        row = connection.execute(
            "SELECT e.definition_block_id, e.definition_offset, e.definition_length "
            "FROM entries e JOIN entry_keys k ON k.entry_id = e.id "
            "WHERE k.kind = 0 AND k.normalized_value = ? LIMIT 1",
            (query,),
        ).fetchone()
        checks[f"kana-navigation:{query}"] = read_definition(row).count("entry://") >= 2

    tags: Counter[str] = Counter()
    link_entries = 0
    sample_step = max(entry_count // 2500, 1)
    for row in connection.execute(
        "SELECT definition_block_id, definition_offset, definition_length "
        "FROM entries WHERE id % ? = 0 LIMIT 2500",
        (sample_step,),
    ):
        definition_text = read_definition(row)
        tags.update(tag.lower() for tag in re.findall(r"<\s*/?\s*([a-zA-Z][\w:-]*)", definition_text))
        link_entries += int("entry://" in definition_text)

    report = {
        "dictionary": str(args.dictionary),
        "entry_count": entry_count,
        "alias_count": alias_count,
        "total_source_entries": entry_count + alias_count,
        "stratified_sample_size": 2500,
        "sample_link_entries": link_entries,
        "sample_tags": dict(tags.most_common(20)),
        "chapter": args.chapter,
        "chapter_characters": len(chapter),
        "source_terms": {term: term in chapter for term in required_source_terms},
        "dictionary_checks": checks,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    if missing_source or not all(checks.values()) or entry_count == 0:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
