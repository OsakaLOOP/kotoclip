"""创建 Kotoclip schema v4 starter `.kdict` 词典源包。"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from dictionary_bundle import build_bundle_from_entries


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


STARTER_ENTRIES = [
    ("警察署", "ケイサツショ", "<div>警察署（けいさつしょ）：警察本部の下部機関。</div>"),
    ("はぐれ者", "ハグレモノ", "<div>はぐれ者（はぐれもの）：仲間から離れた者。</div>"),
    ("古川", "フルカワ", "<div>古川（ふるかわ）：日本の姓。</div>"),
    ("鬼怒川", "キヌガワ", "<div>鬼怒川（きぬがわ）：日本の川の名前、温泉地。</div>"),
    ("煙草", "タバコ", "<div>煙草（たばこ）：タバコ草の葉を加工した嗜好品。</div>"),
    ("食べる", "タベル", "<div>食べる（たべる）：食物を口に入れて咀嚼し、飲み下す。</div>"),
    ("行く", "イク", "<div>行く（いく）：歩み進む。目的地に向かって進む。</div>"),
]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "output",
        nargs="?",
        type=Path,
        default=Path("data/dict-sources/starter.kdict"),
    )
    args = parser.parse_args()
    report = build_bundle_from_entries(STARTER_ENTRIES, args.output, "StarterDict")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
