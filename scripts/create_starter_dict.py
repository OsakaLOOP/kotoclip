import argparse
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path

from dictionary_schema import SCHEMA_VERSION, ensure_schema, rebuild_search_tables

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def create_starter(sqlite_path: Path, force: bool = False):
    sqlite_path.parent.mkdir(parents=True, exist_ok=True)
    if sqlite_path.exists():
        if not force:
            raise FileExistsError(f"目标已存在；如需覆盖请使用 --force：{sqlite_path}")
        sqlite_path.unlink()

    print(f"Creating starter dictionary database at {sqlite_path}...")
    conn = sqlite3.connect(str(sqlite_path))
    cursor = conn.cursor()
    ensure_schema(conn)

    # 1. 创建主表与索引
    # 2. 创建 FTS5 虚拟表及同步触发器
    cursor.execute('''
        CREATE VIRTUAL TABLE entries_fts USING fts5(
            headword,
            definition,
            content='entries',
            content_rowid='id',
            tokenize='trigram'
        )
    ''')

    cursor.execute('''
        CREATE TRIGGER entries_ai AFTER INSERT ON entries BEGIN
            INSERT INTO entries_fts(rowid, headword, definition)
            VALUES (new.id, new.headword, new.definition);
        END;
    ''')
    cursor.execute('''
        CREATE TRIGGER entries_ad AFTER DELETE ON entries BEGIN
            INSERT INTO entries_fts(entries_fts, rowid, headword, definition)
            VALUES('delete', old.id, old.headword, old.definition);
        END;
    ''')
    cursor.execute('''
        CREATE TRIGGER entries_au AFTER UPDATE ON entries BEGIN
            INSERT INTO entries_fts(entries_fts, rowid, headword, definition)
            VALUES('delete', old.id, old.headword, old.definition);
            INSERT INTO entries_fts(rowid, headword, definition)
            VALUES (new.id, new.headword, new.definition);
        END;
    ''')

    # 3. 插入测试与基础词汇
    test_entries = [
        ("警察署", "ケイサツショ", "<div>警察署（けいさつしょ）：警察本部の下部機関。</div>", "StarterDict"),
        ("はぐれ者", "ハグレモノ", "<div>はぐれ者（はぐれもの）：仲間から離れた者。</div>", "StarterDict"),
        ("古川", "フルカワ", "<div>古川（ふるかわ）：日本の姓。</div>", "StarterDict"),
        ("鬼怒川", "キヌガワ", "<div>鬼怒川（きぬがわ）：日本の川の名前、温泉地。</div>", "StarterDict"),
        ("煙草", "タバコ", "<div>煙草（たばこ）：タバコ草の葉を加工した嗜好品。</div>", "StarterDict"),
        ("食べる", "タベル", "<div>食べる（たべる）：食物を口に入れて咀嚼し、飲み下す。</div>", "StarterDict"),
        ("行く", "イク", "<div>行く（いく）：歩み進む。目的地に向かって進む。</div>", "StarterDict"),
    ]

    cursor.executemany(
        'INSERT INTO entries (headword, reading, definition, dict_name) VALUES (?, ?, ?, ?)',
        test_entries
    )

    cursor.execute("INSERT INTO entries_fts(entries_fts) VALUES('rebuild')")
    rebuild_search_tables(conn)
    cursor.execute(
        "INSERT INTO metadata(schema_version, source_name, imported_at) VALUES (?, 'StarterDict', ?)",
        (SCHEMA_VERSION, datetime.now(timezone.utc).isoformat()),
    )

    conn.commit()
    conn.close()
    print("Starter dictionary database created successfully.")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description="创建 Kotoclip schema v3 starter 词典。")
    parser.add_argument("output", nargs="?", type=Path, default=Path("data/dicts/starter.sqlite"))
    parser.add_argument("--force", action="store_true")
    arguments = parser.parse_args()
    create_starter(arguments.output, arguments.force)
