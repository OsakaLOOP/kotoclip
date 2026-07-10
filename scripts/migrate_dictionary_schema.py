import argparse
import shutil
import sqlite3
from pathlib import Path

def migrate(path: Path) -> None:
    backup = path.with_suffix(path.suffix + ".before-reading-migration.bak")
    shutil.copy2(path, backup)
    with sqlite3.connect(path) as conn:
        columns = {row[1] for row in conn.execute("PRAGMA table_info(entries)")}
        if "reading" not in columns:
            conn.execute("ALTER TABLE entries ADD COLUMN reading TEXT")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_entries_reading ON entries(reading)")
        conn.execute("CREATE TABLE IF NOT EXISTS metadata (schema_version INTEGER NOT NULL, source_name TEXT NOT NULL, imported_at TEXT NOT NULL)")
        conn.execute("INSERT INTO metadata(schema_version, source_name, imported_at) VALUES (2, ?, datetime('now'))", (path.name,))

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Add Kotoclip reading/schema metadata to a dictionary database.")
    parser.add_argument("database", type=Path)
    args = parser.parse_args()
    migrate(args.database)
    print(f"Migrated {args.database}; backup: {args.database}.before-reading-migration.bak")
