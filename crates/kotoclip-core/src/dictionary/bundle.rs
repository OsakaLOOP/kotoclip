use flate2::read::ZlibDecoder;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 4;
const MAGIC: &[u8; 8] = b"KDICT\0\x01\0";
const CANONICAL_ENTRY: u8 = 0;
const ALIAS_ENTRY: u8 = 1;

pub(crate) const BASE_SCHEMA: &str = r#"
CREATE TABLE metadata (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    schema_version INTEGER NOT NULL,
    format_version INTEGER NOT NULL,
    source_name TEXT NOT NULL,
    bundle_id TEXT NOT NULL UNIQUE,
    canonical_count INTEGER NOT NULL,
    alias_count INTEGER NOT NULL,
    definition_block_count INTEGER NOT NULL
);
CREATE TABLE definition_blocks (
    id INTEGER PRIMARY KEY,
    uncompressed_size INTEGER NOT NULL,
    data BLOB NOT NULL
);
CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    headword TEXT NOT NULL,
    definition_block_id INTEGER NOT NULL,
    definition_offset INTEGER NOT NULL,
    definition_length INTEGER NOT NULL
);
CREATE TABLE aliases (
    alias TEXT NOT NULL,
    normalized_alias TEXT,
    target TEXT NOT NULL,
    PRIMARY KEY(alias, target)
) WITHOUT ROWID;
CREATE TABLE alias_keys (
    alias TEXT NOT NULL,
    target TEXT NOT NULL,
    kind INTEGER NOT NULL CHECK(kind IN (0, 1)),
    normalized_value TEXT NOT NULL,
    PRIMARY KEY(kind, normalized_value, alias, target)
) WITHOUT ROWID;
CREATE TABLE entry_keys (
    entry_id INTEGER NOT NULL,
    kind INTEGER NOT NULL CHECK(kind IN (0, 1)),
    normalized_value TEXT NOT NULL,
    display_value TEXT,
    rank INTEGER NOT NULL,
    PRIMARY KEY(entry_id, kind, normalized_value)
) WITHOUT ROWID;
"#;

pub(crate) const SEARCH_SCHEMA: &str = r#"
CREATE INDEX idx_entries_headword ON entries(headword);
CREATE INDEX idx_aliases_normalized
    ON aliases(normalized_alias, target)
    WHERE normalized_alias IS NOT NULL;
CREATE INDEX idx_entry_keys_lookup
    ON entry_keys(kind, normalized_value, entry_id);
CREATE VIRTUAL TABLE entries_fts USING fts5(
    headword,
    content='',
    tokenize='trigram',
    detail='none',
    columnsize=0
);
INSERT INTO entries_fts(rowid, headword)
SELECT e.id,
       e.headword || ' ' || COALESCE((
           SELECT group_concat(COALESCE(k.display_value, k.normalized_value), ' ')
           FROM entry_keys k
           WHERE k.entry_id = e.id AND k.kind = 0
       ), '')
FROM entries e;
"#;

#[derive(Debug, Clone, Deserialize)]
pub struct BundleHeader {
    pub format_version: u32,
    pub schema_version: u32,
    pub source_name: String,
    pub bundle_id: String,
    pub canonical_count: usize,
    pub alias_count: usize,
    pub definition_block_count: usize,
    pub metadata_uncompressed_size: usize,
}

struct BundleReader {
    reader: BufReader<File>,
    header: BundleHeader,
    metadata: Vec<u8>,
}

impl BundleReader {
    fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(File::open(path)?);
        let header = read_header_from(&mut reader)?;
        if header.format_version != 1 || header.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "词典源包版本不兼容：format={} schema={}",
                header.format_version, header.schema_version
            )
            .into());
        }
        let metadata_size = read_u64(&mut reader)? as usize;
        let compressed_size = read_u64(&mut reader)? as usize;
        if metadata_size != header.metadata_uncompressed_size {
            return Err("词典源包 metadata 长度与 header 不一致".into());
        }
        let mut compressed = vec![0; compressed_size];
        reader.read_exact(&mut compressed)?;
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let mut metadata = Vec::with_capacity(metadata_size);
        decoder.read_to_end(&mut metadata)?;
        if metadata.len() != metadata_size {
            return Err("词典源包 metadata 解压长度不一致".into());
        }
        Ok(Self {
            reader,
            header,
            metadata,
        })
    }
}

pub fn prepare_dictionary_sources<P1: AsRef<Path>, P2: AsRef<Path>>(
    source_dir: P1,
    database_dir: P2,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let source_dir = source_dir.as_ref();
    let database_dir = database_dir.as_ref();
    fs::create_dir_all(source_dir)?;
    fs::create_dir_all(database_dir)?;

    let mut sources = fs::read_dir(source_dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("kdict")
        })
        .collect::<Vec<_>>();
    sources.sort();

    let mut databases = Vec::with_capacity(sources.len());
    for source in sources {
        let header = read_bundle_header(&source)?;
        let target = database_dir.join(format!(
            "{}.db",
            source
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("dictionary")
        ));
        if database_bundle_id(&target).as_deref() != Some(header.bundle_id.as_str()) {
            build_database(&source, &target)?;
        }
        databases.push(target);
    }
    Ok(databases)
}

pub fn read_bundle_header(path: &Path) -> Result<BundleHeader, Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(File::open(path)?);
    read_header_from(&mut reader)
}

fn read_header_from(reader: &mut impl Read) -> Result<BundleHeader, Box<dyn std::error::Error>> {
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err("不是有效的 Kotoclip 词典源包".into());
    }
    let header_size = read_u32(reader)? as usize;
    let mut header = vec![0; header_size];
    reader.read_exact(&mut header)?;
    Ok(serde_json::from_slice(&header)?)
}

fn database_bundle_id(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let connection = Connection::open(path).ok()?;
    connection
        .query_row(
            "SELECT bundle_id FROM metadata WHERE id = 1 AND schema_version = ?1",
            [SCHEMA_VERSION],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten()
}

pub fn build_database(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut bundle = BundleReader::open(source)?;
    let parent = target
        .parent()
        .ok_or_else(|| format!("数据库目标缺少父目录：{}", target.display()))?;
    fs::create_dir_all(parent)?;
    let temporary = target.with_extension("db.building");
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }

    let mut connection = Connection::open(&temporary)?;
    connection.execute_batch(
        "PRAGMA page_size=8192;
         PRAGMA synchronous=OFF;
         PRAGMA temp_store=MEMORY;",
    )?;
    let _: String = connection.query_row("PRAGMA journal_mode=OFF", [], |row| row.get(0))?;
    let _: String = connection.query_row("PRAGMA locking_mode=EXCLUSIVE", [], |row| row.get(0))?;
    connection.execute_batch(BASE_SCHEMA)?;

    let mut canonical_count = 0usize;
    let mut alias_count = 0usize;
    {
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO metadata(
                id, schema_version, format_version, source_name, bundle_id,
                canonical_count, alias_count, definition_block_count
             ) VALUES(1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                bundle.header.schema_version,
                bundle.header.format_version,
                &bundle.header.source_name,
                &bundle.header.bundle_id,
                bundle.header.canonical_count,
                bundle.header.alias_count,
                bundle.header.definition_block_count,
            ],
        )?;

        let mut entry_statement = transaction.prepare(
            "INSERT INTO entries(
                id, headword, definition_block_id, definition_offset, definition_length
             ) VALUES(?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut alias_statement = transaction
            .prepare("INSERT INTO aliases(alias, normalized_alias, target) VALUES(?1, ?2, ?3)")?;
        let mut alias_key_statement = transaction.prepare(
            "INSERT INTO alias_keys(
                alias, target, kind, normalized_value
             ) VALUES(?1, ?2, ?3, ?4)",
        )?;
        let mut key_statement = transaction.prepare(
            "INSERT INTO entry_keys(
                entry_id, kind, normalized_value, display_value, rank
             ) VALUES(?1, ?2, ?3, ?4, ?5)",
        )?;

        let mut cursor = MetadataCursor::new(&bundle.metadata);
        while !cursor.is_empty() {
            match cursor.read_u8()? {
                CANONICAL_ENTRY => {
                    canonical_count += 1;
                    let headword = cursor.read_text()?;
                    let block_id = cursor.read_u32()?;
                    let offset = cursor.read_u32()?;
                    let length = cursor.read_u32()?;
                    entry_statement.execute(params![
                        canonical_count,
                        headword,
                        block_id,
                        offset,
                        length
                    ])?;
                    insert_keys(&mut cursor, &mut key_statement, canonical_count, 0)?;
                    insert_keys(&mut cursor, &mut key_statement, canonical_count, 1)?;
                }
                ALIAS_ENTRY => {
                    alias_count += 1;
                    let alias = cursor.read_text()?;
                    let normalized_alias = cursor.read_optional_text()?;
                    let target = cursor.read_text()?;
                    alias_statement.execute(params![&alias, normalized_alias, &target])?;
                    insert_alias_keys(&mut cursor, &mut alias_key_statement, &alias, &target, 0)?;
                    insert_alias_keys(&mut cursor, &mut alias_key_statement, &alias, &target, 1)?;
                }
                tag => return Err(format!("未知词典 metadata 记录类型：{tag}").into()),
            }
        }
        drop(entry_statement);
        drop(alias_statement);
        drop(alias_key_statement);
        drop(key_statement);

        if canonical_count != bundle.header.canonical_count
            || alias_count != bundle.header.alias_count
        {
            return Err("词典源包记录数量与 header 不一致".into());
        }

        let block_count = read_u32(&mut bundle.reader)? as usize;
        if block_count != bundle.header.definition_block_count {
            return Err("词典源包 definition block 数量不一致".into());
        }
        let mut block_statement = transaction.prepare(
            "INSERT INTO definition_blocks(id, uncompressed_size, data) VALUES(?1, ?2, ?3)",
        )?;
        for block_id in 1..=block_count {
            let uncompressed_size = read_u32(&mut bundle.reader)?;
            let compressed_size = read_u32(&mut bundle.reader)? as usize;
            let mut data = vec![0; compressed_size];
            bundle.reader.read_exact(&mut data)?;
            block_statement.execute(params![block_id, uncompressed_size, data])?;
        }
        drop(block_statement);
        transaction.commit()?;
    }

    connection.execute_batch(SEARCH_SCHEMA)?;
    connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    connection.execute_batch("ANALYZE; VACUUM;")?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(format!("生成的词典数据库完整性检查失败：{integrity}").into());
    }
    drop(connection);

    if target.exists() {
        fs::remove_file(target)?;
    }
    fs::rename(&temporary, target)?;
    Ok(())
}

fn insert_keys(
    cursor: &mut MetadataCursor<'_>,
    statement: &mut rusqlite::Statement<'_>,
    entry_id: usize,
    kind: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let count = cursor.read_u16()? as usize;
    for _ in 0..count {
        statement.execute(params![
            entry_id,
            kind,
            cursor.read_text()?,
            cursor.read_optional_text()?,
            cursor.read_u16()?,
        ])?;
    }
    Ok(())
}

fn insert_alias_keys(
    cursor: &mut MetadataCursor<'_>,
    statement: &mut rusqlite::Statement<'_>,
    alias: &str,
    target: &str,
    kind: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let count = cursor.read_u16()? as usize;
    for _ in 0..count {
        statement.execute(params![alias, target, kind, cursor.read_text()?,])?;
    }
    Ok(())
}

fn read_u32(reader: &mut impl Read) -> Result<u32, std::io::Error> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, std::io::Error> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

struct MetadataCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> MetadataCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn read_u8(&mut self) -> Result<u8, Box<dyn std::error::Error>> {
        Ok(self.take(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, Box<dyn std::error::Error>> {
        let bytes: [u8; 2] = self.take(2)?.try_into()?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, Box<dyn std::error::Error>> {
        let bytes: [u8; 4] = self.take(4)?.try_into()?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_text(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let size = self.read_u32()? as usize;
        Ok(std::str::from_utf8(self.take(size)?)?.to_string())
    }

    fn read_optional_text(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let size = self.read_u32()?;
        if size == u32::MAX {
            return Ok(None);
        }
        Ok(Some(
            std::str::from_utf8(self.take(size as usize)?)?.to_string(),
        ))
    }

    fn take(&mut self, size: usize) -> Result<&'a [u8], Box<dyn std::error::Error>> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or("词典 metadata 长度溢出")?;
        if end > self.bytes.len() {
            return Err("词典 metadata 提前结束".into());
        }
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }
}
