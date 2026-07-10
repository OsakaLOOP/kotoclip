import sys
import os
import sqlite3

# 尝试导入 readmdict。如果在用户环境中没有安装，将会在运行时报错并引导用户安装。
try:
    from readmdict import MDX
except ImportError:
    print("错误: 未找到 'readmdict' 库。请先在终端运行 'pip install readmdict' 予以安装。")
    sys.exit(1)

def convert_mdx_to_sqlite(mdx_path, sqlite_path, dict_name):
    """
    将 MDX 词典文件解析并导入到本地 SQLite 关系型数据库中，并建立 FTS5 trigram 索引。
    """
    if not os.path.exists(mdx_path):
        print(f"错误: MDX 文件不存在: {mdx_path}")
        return

    print(f"正在加载 MDX 词典: {mdx_path} ...")
    mdx = MDX(mdx_path)
    
    print(f"正在连接目标 SQLite 数据库: {sqlite_path} ...")
    conn = sqlite3.connect(sqlite_path)
    cursor = conn.cursor()

    # 1. 创建词条主表与精确索引
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
    # 注意: trigram 分词器在 SQLite 3.34.0+ 版本可用，常用于中日韩等无空格分词语言的子串模糊检索
    cursor.execute('''
        CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
            headword,
            definition,
            content='entries',
            content_rowid='id',
            tokenize='trigram'
        )
    ''')

    # 3. 创建数据同步触发器 (用于主表增删改时自动更新全文检索索引)
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

    # 4. 读取并流式批量插入数据
    print("正在提取并导入词条...")
    insert_data = []
    count = 0

    for key, value in mdx.items():
        # 解密并转换编码为 utf-8 字符串
        headword = key.decode('utf-8', errors='ignore').strip()
        definition = value.decode('utf-8', errors='ignore').strip()
        
        # 过滤空词条
        if not headword or not definition:
            continue
            
        # MDX metadata is not a reliable structured source for all dictionaries;
        # leave reading NULL instead of guessing from definition prose.
        insert_data.append((headword, None, definition, dict_name))
        count += 1
        
        if len(insert_data) >= 5000:
            cursor.executemany(
                'INSERT INTO entries (headword, reading, definition, dict_name) VALUES (?, ?, ?, ?)',
                insert_data
            )
            conn.commit()
            print(f"已导入 {count} 条...")
            insert_data = []

    if insert_data:
        cursor.executemany(
            'INSERT INTO entries (headword, reading, definition, dict_name) VALUES (?, ?, ?, ?)',
            insert_data
        )
        conn.commit()

    print(f"导入完成! 共处理 {count} 个词条。")

    # 5. 执行 FTS5 rebuild 以填充虚拟表 (若初次建表导入)
    print("正在构建 FTS5 全文检索索引...")
    cursor.execute("INSERT INTO entries_fts(entries_fts) VALUES('rebuild')")
    cursor.execute("INSERT INTO metadata(schema_version, source_name, imported_at) VALUES (2, ?, datetime('now'))", (dict_name,))
    conn.commit()
    
    conn.close()
    print("数据库转换与优化成功。")

if __name__ == '__main__':
    if len(sys.argv) < 4:
        print("用法: python mdx_to_sqlite.py <mdx文件路径> <sqlite文件路径> <自定义词典名>")
        sys.exit(1)
        
    mdx_path = sys.argv[1]
    sqlite_path = sys.argv[2]
    dict_name = sys.argv[3]
    
    convert_mdx_to_sqlite(mdx_path, sqlite_path, dict_name)
