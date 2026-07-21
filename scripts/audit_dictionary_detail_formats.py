"""审计三本词典中直接影响释义排版的原始 HTML 格式族。"""

from __future__ import annotations

import argparse
import html
import json
import re
import sqlite3
import sys
import zlib
from collections import Counter, defaultdict
from collections.abc import Iterator
from pathlib import Path


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


MEANING_PARAGRAPH_RE = re.compile(
    r'<p\b[^>]*data-orgtag="meaning"[^>]*>.*?</p>', re.S
)
ATTRIBUTE_RE = re.compile(r'([\w-]+)="([^"]*)"')
HEADING_RE = re.compile(
    r'<span\b[^>]*type="語義区分2"[^>]*>(.*?)</span>', re.S
)
INLINE_TAG_RE = re.compile(
    r'<(?:span|b)\b[^>]*class="[^"]*(?:white-square|black-square)[^"]*"[^>]*>'
    r'(.*?)</(?:span|b)>',
    re.S,
)
SQUARE_QUALIFIER_RE = re.compile(r'［([^］]+)］')
TAG_RE = re.compile(r'<[^>]+>')
JAPANESE_KANA_RE = re.compile(r'[ぁ-ゖァ-ヺー]')
STANDALONE_SUBHEAD_RE = re.compile(r'data-orgtag="subhead"')
SUBHEAD_TYPE_RE = re.compile(r'data-orgtag="subhead"[^>]*\btype="([^"]+)"')


def iter_entries(path: Path) -> Iterator[tuple[str, str]]:
    """按数据库记录中的 UTF-8 字节偏移读取原始定义。"""
    connection = sqlite3.connect(path)
    try:
        blocks = {
            block_id: zlib.decompress(data)
            for block_id, data in connection.execute(
                "SELECT id, data FROM definition_blocks"
            )
        }
        for headword, block_id, offset, length in connection.execute(
            "SELECT headword, definition_block_id, definition_offset, definition_length "
            "FROM entries ORDER BY id"
        ):
            yield str(headword), blocks[block_id][offset : offset + length].decode(
                "utf-8"
            )
    finally:
        connection.close()


def clean_fragment(value: str, limit: int = 700) -> str:
    value = re.sub(r"\s+", " ", value).strip()
    return value if len(value) <= limit else f"{value[:limit]}…"


def add_sample(
    samples: dict[str, list[dict[str, str]]],
    kind: str,
    headword: str,
    fragment: str,
    limit: int,
) -> None:
    if len(samples[kind]) < limit:
        samples[kind].append(
            {"headword": headword, "html": clean_fragment(fragment)}
        )


def audit_shogakukan(path: Path, sample_limit: int) -> dict[str, object]:
    counts: Counter[str] = Counter()
    labels: Counter[str] = Counter()
    standalone_subhead_types: Counter[str] = Counter()
    samples: dict[str, list[dict[str, str]]] = defaultdict(list)
    entry_count = 0
    for headword, raw in iter_entries(path):
        entry_count += 1
        if "<h3" not in raw and STANDALONE_SUBHEAD_RE.search(raw):
            counts["standalone_subhead_entries"] += 1
            subhead_type = SUBHEAD_TYPE_RE.search(raw)
            standalone_subhead_types[
                subhead_type.group(1) if subhead_type else "(none)"
            ] += 1
            add_sample(
                samples,
                "standalone_subhead_entries",
                headword,
                raw,
                sample_limit,
            )
        parsed = []
        for paragraph_match in MEANING_PARAGRAPH_RE.finditer(raw):
            paragraph = paragraph_match.group(0)
            visible = html.unescape(TAG_RE.sub("", paragraph))
            square_qualifiers = [
                value
                for value in SQUARE_QUALIFIER_RE.findall(visible)
                if JAPANESE_KANA_RE.search(value)
            ]
            inline_tags = [
                TAG_RE.sub("", value).strip()
                for value in INLINE_TAG_RE.findall(paragraph)
                if TAG_RE.sub("", value).strip()
            ]
            if len(square_qualifiers) >= 2:
                counts["multiple_japanese_square_qualifiers"] += 1
                add_sample(
                    samples,
                    "multiple_japanese_square_qualifiers",
                    headword,
                    paragraph,
                    sample_limit,
                )
            if paragraph.count("；") + paragraph.count(";") >= 2:
                counts["multiple_semicolons"] += 1
                add_sample(
                    samples,
                    "multiple_semicolons",
                    headword,
                    paragraph,
                    sample_limit,
                )
            if re.search(r"[（(][^）)]*<b>[^<]+</b>[^）)]*[）)]", paragraph):
                counts["parenthetical_bold"] += 1
                add_sample(
                    samples,
                    "parenthetical_bold",
                    headword,
                    paragraph,
                    sample_limit,
                )
            if inline_tags:
                counts["inline_structural_tags"] += 1
                labels.update(inline_tags)
                add_sample(
                    samples,
                    "inline_structural_tags",
                    headword,
                    paragraph,
                    sample_limit,
                )

            attrs = dict(ATTRIBUTE_RE.findall(paragraph))
            heading_match = HEADING_RE.search(paragraph)
            if attrs.get("level") and attrs.get("no") and heading_match:
                parsed.append(
                    (
                        attrs["level"],
                        attrs["no"],
                        TAG_RE.sub("", heading_match.group(1)).strip(),
                        paragraph,
                    )
                )
        for previous, current in zip(parsed, parsed[1:]):
            if (
                previous[0] == current[0]
                and previous[1] == current[1]
                and previous[2] != current[2]
            ):
                counts["repeated_marker_gloss_groups"] += 1
                add_sample(
                    samples,
                    "repeated_marker_gloss_groups",
                    headword,
                    f"{previous[3]}\n{current[3]}",
                    sample_limit,
                )

        for literal, key in [
            ("〈法〉", "domain_label_law"),
            ("成語", "label_idiom"),
            ("書面語", "label_written"),
            ("口語", "label_spoken"),
        ]:
            counts[key] += raw.count(literal)
    return {
        "entries": entry_count,
        "counts": dict(counts),
        "inline_tag_labels": dict(labels.most_common()),
        "standalone_subhead_types": dict(standalone_subhead_types.most_common()),
        "samples": dict(samples),
    }


def audit_daijirin(path: Path, sample_limit: int) -> dict[str, object]:
    patterns = {
        "small_pos_markup": re.compile(r'（動<span class="small">'),
        "possible_sections": re.compile(r">可能<"),
        "derivation_sections": re.compile(r">派生<"),
        "multiple_major_groups": re.compile(r'type="invert-rect"'),
    }
    counts: Counter[str] = Counter()
    samples: dict[str, list[dict[str, str]]] = defaultdict(list)
    entry_count = 0
    for headword, raw in iter_entries(path):
        entry_count += 1
        for kind, pattern in patterns.items():
            matches = pattern.findall(raw)
            if not matches:
                continue
            counts[kind] += 1
            add_sample(samples, kind, headword, raw, sample_limit)
    return {"entries": entry_count, "counts": dict(counts), "samples": dict(samples)}


def audit_crown(path: Path, sample_limit: int) -> dict[str, object]:
    counts: Counter[str] = Counter()
    samples: dict[str, list[dict[str, str]]] = defaultdict(list)
    entry_count = 0
    for headword, raw in iter_entries(path):
        entry_count += 1
        if "mean_kubun" in raw and "mean_yakugo" in raw:
            counts["heading_with_chinese_gloss"] += 1
            add_sample(
                samples,
                "heading_with_chinese_gloss",
                headword,
                raw,
                sample_limit,
            )
        example_count = raw.count('class="mean_yoreiyaku"')
        if example_count > 2:
            counts["entries_with_more_than_two_examples"] += 1
            add_sample(
                samples,
                "entries_with_more_than_two_examples",
                headword,
                raw,
                sample_limit,
            )
    return {"entries": entry_count, "counts": dict(counts), "samples": dict(samples)}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dict-dir", type=Path, default=Path("data/dicts"))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--sample-limit", type=int, default=5)
    args = parser.parse_args()

    result = {
        "shogakukan": audit_shogakukan(
            args.dict_dir / "shogakukan.db", args.sample_limit
        ),
        "daijirin": audit_daijirin(args.dict_dir / "daijirin.db", args.sample_limit),
        "crown": audit_crown(args.dict_dir / "crown.db", args.sample_limit),
    }
    serialized = json.dumps(result, ensure_ascii=False, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized, encoding="utf-8")
        print(f"审计结果已保存至：{args.output}")
    else:
        print(serialized)


if __name__ == "__main__":
    main()
