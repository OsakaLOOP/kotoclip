import argparse
import shutil
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path

from dictionary_schema import SCHEMA_VERSION, rebuild_search_tables

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

def migrate(path: Path) -> None:
    backup = path.with_suffix(path.suffix + ".before-schema-v3.bak")
    if not backup.exists():
        shutil.copy2(path, backup)
    with sqlite3.connect(path) as conn:
        rebuild_search_tables(conn)
        conn.execute(
            "INSERT INTO metadata(schema_version, source_name, imported_at) VALUES (?, ?, ?)",
            (SCHEMA_VERSION, path.name, datetime.now(timezone.utc).isoformat()),
        )

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="将 Kotoclip 词典升级到 schema v3，并建立表记／读音索引。")
    parser.add_argument("database", type=Path)
    args = parser.parse_args()
    migrate(args.database)
    print(f"升级完成：{args.database}；备份：{args.database}.before-schema-v3.bak")
