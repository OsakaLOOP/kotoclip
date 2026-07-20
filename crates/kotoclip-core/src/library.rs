use crate::import::epub::{import_epub, ImportedEpub};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const LIBRARY_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct ReaderLibrary {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBookSummary {
    pub id: String,
    pub title: String,
    pub author: String,
    pub language: String,
    pub source_name: String,
    pub cover_path: Option<String>,
    pub chapter_count: usize,
    pub total_characters: usize,
    pub progress_offset: usize,
    pub progress_percent: f64,
    pub current_chapter: Option<String>,
    pub created_at: String,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryResource {
    pub href: String,
    pub path: String,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBook {
    #[serde(flatten)]
    pub summary: LibraryBookSummary,
    pub markdown: String,
    pub chapter_titles: Vec<String>,
    pub resources: Vec<LibraryResource>,
    pub warnings: Vec<String>,
    pub library_path: String,
}

impl ReaderLibrary {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let library = Self {
            root: root.as_ref().to_path_buf(),
        };
        std::fs::create_dir_all(library.root.join("books"))?;
        let connection = library.connection()?;
        initialize_schema(&connection)?;
        Ok(library)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn list_books(&self) -> Result<Vec<LibraryBookSummary>, Box<dyn std::error::Error>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, title, author, language, source_name, cover_path,
                    chapter_count, total_characters, progress_offset, progress_percent,
                    current_chapter, created_at, last_opened_at
             FROM books
             ORDER BY CASE WHEN last_opened_at IS NULL THEN 1 ELSE 0 END,
                      last_opened_at DESC, created_at DESC",
        )?;
        let rows = statement.query_map([], read_book_summary)?;
        let mut books = rows.collect::<Result<Vec<_>, _>>()?;
        for book in &mut books {
            book.cover_path = book
                .cover_path
                .take()
                .map(|path| self.root.join(path).to_string_lossy().into_owned());
        }
        Ok(books)
    }

    pub fn import_epub(
        &self,
        source_path: impl AsRef<Path>,
    ) -> Result<LibraryBook, Box<dyn std::error::Error>> {
        let source_path = source_path.as_ref();
        let id = content_id(source_path)?;
        let imported = import_epub(source_path)?;
        let warnings = imported.warnings.clone();
        self.persist_imported(&id, source_path, imported)?;
        let mut book = self.open_book(&id)?;
        book.warnings = warnings;
        Ok(book)
    }

    pub fn open_book(&self, id: &str) -> Result<LibraryBook, Box<dyn std::error::Error>> {
        validate_book_id(id)?;
        let now = Utc::now().to_rfc3339();
        let connection = self.connection()?;
        connection.execute(
            "UPDATE books SET last_opened_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        let mut summary = connection
            .query_row(
                "SELECT id, title, author, language, source_name, cover_path,
                        chapter_count, total_characters, progress_offset, progress_percent,
                        current_chapter, created_at, last_opened_at
                 FROM books WHERE id = ?1",
                [id],
                read_book_summary,
            )
            .optional()?
            .ok_or_else(|| format!("书库中不存在书籍：{id}"))?;
        summary.cover_path = summary
            .cover_path
            .take()
            .map(|path| self.root.join(path).to_string_lossy().into_owned());
        let markdown = std::fs::read_to_string(self.root.join("books").join(id).join("content.md"))?;
        let chapter_titles = self.chapter_titles(&connection, id)?;
        let resources = self.resources(&connection, id)?;
        Ok(LibraryBook {
            summary,
            markdown,
            chapter_titles,
            resources,
            warnings: Vec::new(),
            library_path: self.root.to_string_lossy().into_owned(),
        })
    }

    pub fn update_progress(
        &self,
        id: &str,
        progress_offset: usize,
        total_characters: usize,
        current_chapter: Option<&str>,
        reading_seconds: u64,
    ) -> Result<LibraryBookSummary, Box<dyn std::error::Error>> {
        validate_book_id(id)?;
        let bounded_offset = progress_offset.min(total_characters);
        let progress_percent = if total_characters == 0 {
            0.0
        } else {
            bounded_offset as f64 / total_characters as f64
        };
        let connection = self.connection()?;
        connection.execute(
            "UPDATE books
             SET progress_offset = ?2, progress_percent = ?3, total_characters = ?4,
                 current_chapter = ?5, reading_seconds = reading_seconds + ?6, updated_at = ?7
             WHERE id = ?1",
            params![
                id,
                bounded_offset as i64,
                progress_percent,
                total_characters as i64,
                current_chapter,
                reading_seconds as i64,
                Utc::now().to_rfc3339(),
            ],
        )?;
        let mut summary = connection.query_row(
            "SELECT id, title, author, language, source_name, cover_path,
                    chapter_count, total_characters, progress_offset, progress_percent,
                    current_chapter, created_at, last_opened_at
             FROM books WHERE id = ?1",
            [id],
            read_book_summary,
        )?;
        summary.cover_path = summary
            .cover_path
            .take()
            .map(|path| self.root.join(path).to_string_lossy().into_owned());
        Ok(summary)
    }

    pub fn remove_book(&self, id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        validate_book_id(id)?;
        let book_dir = self.root.join("books").join(id);
        let trash_dir = self.root.join(format!(".removing-{id}"));
        let moved = if book_dir.exists() {
            std::fs::rename(&book_dir, &trash_dir)?;
            true
        } else {
            false
        };
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let deleted = transaction.execute("DELETE FROM books WHERE id = ?1", [id])? > 0;
        if let Err(error) = transaction.commit() {
            if moved {
                let _ = std::fs::rename(&trash_dir, &book_dir);
            }
            return Err(error.into());
        }
        if moved {
            std::fs::remove_dir_all(trash_dir)?;
        }
        Ok(deleted)
    }

    fn connection(&self) -> Result<Connection, rusqlite::Error> {
        let connection = Connection::open(self.root.join("library.sqlite"))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(connection)
    }

    fn persist_imported(
        &self,
        id: &str,
        source_path: &Path,
        imported: ImportedEpub,
    ) -> Result<(), Box<dyn std::error::Error>> {
        validate_book_id(id)?;
        let book_dir = self.root.join("books").join(id);
        let asset_dir = book_dir.join("assets");
        std::fs::create_dir_all(&asset_dir)?;
        std::fs::copy(source_path, book_dir.join("source.epub"))?;
        std::fs::write(book_dir.join("content.md"), imported.markdown.as_bytes())?;

        let mut stored_resources = Vec::with_capacity(imported.resources.len());
        for (index, resource) in imported.resources.iter().enumerate() {
            let extension = safe_extension(&resource.href);
            let file_name = format!("{index:04}.{extension}");
            let relative_path = PathBuf::from("books")
                .join(id)
                .join("assets")
                .join(&file_name);
            std::fs::write(self.root.join(&relative_path), &resource.bytes)?;
            stored_resources.push((resource, relative_path));
        }

        let cover_path = stored_resources
            .iter()
            .find(|(resource, _)| resource.href.eq_ignore_ascii_case("cover.jpeg"))
            .or_else(|| stored_resources.first())
            .map(|(_, path)| path.to_string_lossy().replace('\\', "/"));
        let now = Utc::now().to_rfc3339();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO books (
                id, title, author, language, source_name, format, cover_path,
                chapter_count, total_characters, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'epub', ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                author = excluded.author,
                language = excluded.language,
                source_name = excluded.source_name,
                cover_path = excluded.cover_path,
                chapter_count = excluded.chapter_count,
                total_characters = MAX(books.total_characters, excluded.total_characters),
                updated_at = excluded.updated_at",
            params![
                id,
                imported.title,
                imported.author,
                imported.language,
                imported.source_name,
                cover_path,
                imported.chapter_titles.len() as i64,
                imported.markdown.chars().count() as i64,
                now,
            ],
        )?;
        transaction.execute("DELETE FROM chapters WHERE book_id = ?1", [id])?;
        transaction.execute("DELETE FROM resources WHERE book_id = ?1", [id])?;
        for (index, title) in imported.chapter_titles.iter().enumerate() {
            transaction.execute(
                "INSERT INTO chapters (book_id, position, title) VALUES (?1, ?2, ?3)",
                params![id, index as i64, title],
            )?;
        }
        for (index, (resource, relative_path)) in stored_resources.iter().enumerate() {
            transaction.execute(
                "INSERT INTO resources (book_id, position, href, relative_path, media_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    index as i64,
                    resource.href,
                    relative_path.to_string_lossy(),
                    resource.media_type,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn chapter_titles(
        &self,
        connection: &Connection,
        id: &str,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let mut statement = connection.prepare(
            "SELECT title FROM chapters WHERE book_id = ?1 ORDER BY position",
        )?;
        let rows = statement.query_map([id], |row| row.get(0))?;
        rows.collect()
    }

    fn resources(
        &self,
        connection: &Connection,
        id: &str,
    ) -> Result<Vec<LibraryResource>, rusqlite::Error> {
        let mut statement = connection.prepare(
            "SELECT href, relative_path, media_type
             FROM resources WHERE book_id = ?1 ORDER BY position",
        )?;
        let rows = statement.query_map([id], |row| {
            let relative_path: String = row.get(1)?;
            Ok(LibraryResource {
                href: row.get(0)?,
                path: self.root.join(relative_path).to_string_lossy().into_owned(),
                media_type: row.get(2)?,
            })
        })?;
        rows.collect()
    }
}

fn initialize_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS books (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            author TEXT NOT NULL,
            language TEXT NOT NULL,
            source_name TEXT NOT NULL,
            format TEXT NOT NULL,
            cover_path TEXT,
            chapter_count INTEGER NOT NULL DEFAULT 0,
            total_characters INTEGER NOT NULL DEFAULT 0,
            progress_offset INTEGER NOT NULL DEFAULT 0,
            progress_percent REAL NOT NULL DEFAULT 0,
            current_chapter TEXT,
            reading_seconds INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_opened_at TEXT
        );
        CREATE TABLE IF NOT EXISTS chapters (
            book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            title TEXT NOT NULL,
            PRIMARY KEY (book_id, position)
        );
        CREATE TABLE IF NOT EXISTS resources (
            book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            href TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            media_type TEXT NOT NULL,
            PRIMARY KEY (book_id, href),
            UNIQUE (book_id, position)
        );
        CREATE INDEX IF NOT EXISTS idx_books_last_opened ON books(last_opened_at DESC);",
    )?;
    connection.pragma_update(None, "user_version", LIBRARY_SCHEMA_VERSION)?;
    Ok(())
}

fn read_book_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryBookSummary> {
    Ok(LibraryBookSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        language: row.get(3)?,
        source_name: row.get(4)?,
        cover_path: row.get(5)?,
        chapter_count: row.get::<_, i64>(6)?.max(0) as usize,
        total_characters: row.get::<_, i64>(7)?.max(0) as usize,
        progress_offset: row.get::<_, i64>(8)?.max(0) as usize,
        progress_percent: row.get::<_, f64>(9)?.clamp(0.0, 1.0),
        current_chapter: row.get(10)?,
        created_at: row.get(11)?,
        last_opened_at: row.get(12)?,
    })
}

fn content_id(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn validate_book_id(id: &str) -> Result<(), Box<dyn std::error::Error>> {
    if id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("书籍 ID 无效".into())
    }
}

fn safe_extension(href: &str) -> String {
    let extension = Path::new(href)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpeg" | "jpg" | "png" | "gif" | "webp" | "svg" => extension,
        _ => "bin".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::epub::ImportedEpubResource;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn persists_catalog_files_resources_and_progress() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("kotoclip-library-{nonce}"));
        let source = root.join("fixture.epub");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, b"fixture-source").unwrap();
        let library = ReaderLibrary::open(&root).unwrap();
        let id = content_id(&source).unwrap();
        library
            .persist_imported(
                &id,
                &source,
                ImportedEpub {
                    source_name: "fixture.epub".to_string(),
                    title: "测试书".to_string(),
                    author: "作者".to_string(),
                    date: "2026-07-20".to_string(),
                    language: "ja".to_string(),
                    markdown: "## 第一章\n\n正文。".to_string(),
                    chapter_titles: vec!["第一章".to_string()],
                    resources: vec![ImportedEpubResource {
                        href: "cover.jpeg".to_string(),
                        media_type: "image/jpeg".to_string(),
                        bytes: b"cover".to_vec(),
                    }],
                    warnings: Vec::new(),
                },
            )
            .unwrap();

        let books = library.list_books().unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "测试书");
        let book = library.open_book(&id).unwrap();
        assert_eq!(book.chapter_titles, vec!["第一章"]);
        assert_eq!(std::fs::read(&book.resources[0].path).unwrap(), b"cover");
        let updated = library
            .update_progress(&id, 50, 100, Some("第一章"), 120)
            .unwrap();
        assert_eq!(updated.progress_percent, 0.5);
        assert!(library.remove_book(&id).unwrap());
        assert!(library.list_books().unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
