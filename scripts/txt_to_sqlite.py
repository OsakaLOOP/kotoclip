import sys
import os
import sqlite3
import unicodedata

def normalize_reading(value):
    value = unicodedata.normalize('NFKC', value)
    return ''.join(chr(ord(c) + 0x60) if '\u3041' <= c <= '\u3096' else c for c in value)

def convert_txt_to_sqlite(txt_path, sqlite_path, dict_name):
    if not os.path.exists(txt_path):
        print(f"错误: 文本文件不存在: {txt_path}")
        return

    # 创建目标数据库文件夹 (如果不存在)
    db_dir = os.path.dirname(sqlite_path)
    if db_dir and not os.path.exists(db_dir):
        os.makedirs(db_dir, exist_ok=True)

    print(f"Connecting to target SQLite database: {sqlite_path} ...")
    conn = sqlite3.connect(sqlite_path)
    cursor = conn.cursor()

    # 1. 创建主表与精确查询索引
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            headword TEXT NOT NULL,
            reading TEXT,
            definition TEXT NOT NULL,
            dict_name TEXT NOT NULL
        )
    ''')
    cursor.execute('CREATE INDEX IF NOT EXISTS idx_entries_headword ON entries(headword)')
    cursor.execute('CREATE INDEX IF NOT EXISTS idx_entries_reading ON entries(reading)')
    cursor.execute('CREATE TABLE IF NOT EXISTS metadata (schema_version INTEGER NOT NULL, source_name TEXT NOT NULL, imported_at TEXT NOT NULL)')

    # 2. 创建 FTS5 trigram 全文检索虚拟表
    cursor.execute('''
        CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
            headword,
            definition,
            content='entries',
            content_rowid='id',
            tokenize='trigram'
        )
    ''')

    # 3. 创建数据同步触发器
    cursor.execute('''
        CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
            INSERT INTO entries_fts(rowid, headword, definition) 
            VALUES (new.id, new.headword, new.definition);
        END;
    ''')
    cursor.execute('''
        CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
            INSERT INTO entries_fts(entries_fts, rowid, headword, definition) 
            VALUES('delete', old.id, old.headword, old.definition);
        END;
    ''')
    cursor.execute('''
        CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE ON entries BEGIN
            INSERT INTO entries_fts(entries_fts, rowid, headword, definition) 
            VALUES('delete', old.id, old.headword, old.definition);
            INSERT INTO entries_fts(rowid, headword, definition) 
            VALUES (new.id, new.headword, new.definition);
        END;
    ''')

    print("Parsing and importing entries in stream mode...")
    
    insert_data = []
    count = 0
    
    # 状态机：'head' 代表正在读取词头，'body' 代表正在读取释义
    state = 'head'
    current_headword = ''
    current_definition_lines = []

    # 按行流式读取巨型文本文件 (UTF-8 编码)
    with open(txt_path, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            line_str = line.strip('\r\n')
            
            if state == 'head':
                parts = line_str.split('\t', 1)
                current_headword = parts[0].strip()
                current_reading = normalize_reading(parts[1].strip()) if len(parts) == 2 and parts[1].strip() else None
                if current_headword:  # 忽略开头的空行
                    state = 'body'
                    current_definition_lines = []
            elif state == 'body':
                if line_str == '</>':
                    # 结算当前词条
                    definition = '\n'.join(current_definition_lines).strip()
                    if current_headword and definition:
                        insert_data.append((current_headword, current_reading, definition, dict_name))
                        count += 1
                        
                        if len(insert_data) >= 10000:
                            cursor.executemany(
                                'INSERT INTO entries (headword, reading, definition, dict_name) VALUES (?, ?, ?, ?)',
                                insert_data
                            )
                            conn.commit()
                            print(f"Imported {count} entries...")
                            insert_data = []
                    
                    # 归位状态，继续读取下一个词头
                    state = 'head'
                    current_headword = ''
                else:
                    current_definition_lines.append(line_str)

    # 导入余下的数据
    if insert_data:
        cursor.executemany(
            'INSERT INTO entries (headword, reading, definition, dict_name) VALUES (?, ?, ?, ?)',
            insert_data
        )
        conn.commit()

    print(f"Import completed! Processed {count} entries in total.")

    # 4. 构建全文检索索引
    print("Building FTS5 full-text search index...")
    cursor.execute("INSERT INTO entries_fts(entries_fts) VALUES('rebuild')")
    cursor.execute("INSERT INTO metadata(schema_version, source_name, imported_at) VALUES (2, ?, datetime('now'))", (dict_name,))
    conn.commit()
    
    conn.close()
    print("Database conversion and optimization successful.")

if __name__ == '__main__':
    if len(sys.argv) < 4:
        print("用法: python txt_to_sqlite.py <mdx解压的txt路径> <输出sqlite路径> <自定义词典名>")
        sys.exit(1)
        
    txt_path = sys.argv[1]
    sqlite_path = sys.argv[2]
    dict_name = sys.argv[3]
    
    convert_txt_to_sqlite(txt_path, sqlite_path, dict_name)
