"""为现有 Kotoclip SQLite 词典建立 schema v3 表记／读音索引。"""

from __future__ import annotations

import argparse
import shutil
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path

from dictionary_schema import SCHEMA_VERSION, rebuild_search_tables


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("database", type=Path, help="待升级的 SQLite 词典")
    parser.add_argument("--batch-size", type=int, default=10_000)
    parser.add_argument("--no-backup", action="store_true", help="不建立备份；仅建议用于可重建数据库")
    args = parser.parse_args()

    database = args.database.resolve()
    if not database.is_file():
        parser.error(f"词典不存在：{database}")
    if not args.no_backup:
        backup = database.with_suffix(database.suffix + ".schema-v2.bak")
        if not backup.exists():
            print(f"建立备份：{backup}")
            shutil.copy2(database, backup)

    connection = sqlite3.connect(database)
    try:
        scanned, forms, readings = rebuild_search_tables(connection, args.batch_size)
        source_row = connection.execute(
            "SELECT source_name FROM metadata ORDER BY rowid DESC LIMIT 1"
        ).fetchone()
        source_name = source_row[0] if source_row else database.stem
        connection.execute(
            "INSERT INTO metadata(schema_version, source_name, imported_at) VALUES (?, ?, ?)",
            (SCHEMA_VERSION, source_name, datetime.now(timezone.utc).isoformat()),
        )
        connection.commit()
    finally:
        connection.close()
    print(f"完成：扫描 {scanned} 个词条，写入 {forms} 个表记、{readings} 个读音。")


if __name__ == "__main__":
    main()
