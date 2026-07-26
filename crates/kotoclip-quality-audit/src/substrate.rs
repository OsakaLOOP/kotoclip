use crate::fingerprint::{sha256_bytes, sha256_file, sha256_serializable};
use crate::model::{
    BookChunk, CharRange, ClauseRecord, CorpusSpec, SubstrateBookDescriptor, SubstrateFingerprint,
    SubstrateManifest, SUBSTRATE_SCHEMA_VERSION,
};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use kotoclip_core::pipeline::morpheme::MorphemeAnalyzer;
use kotoclip_core::pipeline::{
    ruby, segment_prepared_text, ProductionSegmentKind, TEXT_BOUNDARY_PROTOCOL_VERSION,
};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct BuildSubstrateOptions {
    pub corpus_spec: PathBuf,
    pub system_dictionary: PathBuf,
    pub output_root: PathBuf,
    pub max_bytes: u64,
}

pub fn freeze_library_corpus(library_root: &Path, output_path: &Path) -> Result<()> {
    let database_path = library_root.join("library.sqlite");
    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("无法只读打开书库 {}", database_path.display()))?;
    let mut statement = connection.prepare("SELECT id FROM books ORDER BY id")?;
    let books: Vec<_> = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .map(|book_id| {
            let book_id = book_id?;
            Ok(crate::model::BookSpec {
                path: library_root.join("books").join(&book_id).join("content.md"),
                book_id,
            })
        })
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
    if books.is_empty() {
        bail!("用户书库没有可审计书籍");
    }
    for book in &books {
        if !book.path.is_file() {
            bail!("书库记录缺少正文：{}", book.path.display());
        }
    }
    let corpus = CorpusSpec {
        schema_version: "kotoclip.quality.corpus.v1".to_string(),
        books,
    };
    let parent = output_path.parent().context("语料清单必须有父目录")?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staging = parent.join(format!(
        ".{}.staging-{}-{nonce}",
        output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let mut writer = BufWriter::new(File::create(&staging)?);
        serde_json::to_writer_pretty(&mut writer, &corpus)?;
        writer.flush()?;
        if output_path.exists() {
            bail!("冻结语料清单已存在：{}", output_path.display());
        }
        fs::rename(&staging, output_path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

pub fn build_substrate(options: &BuildSubstrateOptions) -> Result<PathBuf> {
    let spec_bytes = fs::read(&options.corpus_spec)
        .with_context(|| format!("无法读取语料清单 {}", options.corpus_spec.display()))?;
    let spec: CorpusSpec = serde_json::from_slice(&spec_bytes)?;
    validate_corpus_spec(&spec)?;
    let spec_hash = sha256_bytes(&spec_bytes);
    let fingerprint = SubstrateFingerprint {
        system_dictionary_sha256: sha256_file(&options.system_dictionary)?,
        prepare_text_protocol: ruby::PREPARE_TEXT_PROTOCOL_VERSION.to_string(),
        boundary_protocol: TEXT_BOUNDARY_PROTOCOL_VERSION.to_string(),
        morpheme_compatibility_protocol:
            kotoclip_core::pipeline::morpheme::MORPHEME_COMPATIBILITY_VERSION.to_string(),
        substrate_schema: SUBSTRATE_SCHEMA_VERSION.to_string(),
    };
    let substrate_id = sha256_serializable(&(spec_hash.as_str(), &fingerprint))?;
    let final_directory = options.output_root.join(&substrate_id);
    if final_directory.join("manifest.json").is_file() {
        validate_manifest(&final_directory.join("manifest.json"), &substrate_id)?;
        enforce_substrate_root_capacity(&options.output_root, &substrate_id, options.max_bytes)?;
        return Ok(final_directory);
    }

    fs::create_dir_all(&options.output_root)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staging = options.output_root.join(format!(
        ".{substrate_id}.staging-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&staging)?;
    let result = build_into(
        &staging,
        &options.corpus_spec,
        &spec,
        spec_hash,
        fingerprint,
        substrate_id.clone(),
        &options.system_dictionary,
        options.max_bytes,
    );
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, &final_directory).with_context(|| {
        format!(
            "无法原子发布底座 {} -> {}",
            staging.display(),
            final_directory.display()
        )
    })?;
    enforce_substrate_root_capacity(&options.output_root, &substrate_id, options.max_bytes)?;
    Ok(final_directory)
}

fn enforce_substrate_root_capacity(root: &Path, current_id: &str, max_bytes: u64) -> Result<()> {
    let keep = HashSet::from([current_id.to_string()]);
    gc_substrates(root, &keep, max_bytes)?;
    let retained = directory_size(root)?;
    if retained > max_bytes {
        bail!(
            "当前 IPADIC 底座超过根目录总容量上限：{} > {} 字节",
            retained,
            max_bytes
        );
    }
    Ok(())
}

fn build_into(
    staging: &Path,
    spec_path: &Path,
    spec: &CorpusSpec,
    spec_hash: String,
    fingerprint: SubstrateFingerprint,
    substrate_id: String,
    system_dictionary: &Path,
    max_bytes: u64,
) -> Result<()> {
    let analyzer =
        MorphemeAnalyzer::new(system_dictionary).map_err(|error| anyhow!(error.to_string()))?;
    let spec_base = spec_path.parent().unwrap_or_else(|| Path::new("."));
    let chunk_directory = staging.join("books");
    fs::create_dir(&chunk_directory)?;
    let mut descriptors = Vec::new();
    let mut total_characters = 0;
    let mut total_morphemes = 0;
    let mut total_bytes = 0_u64;

    for (book_index, book) in spec.books.iter().enumerate() {
        let source_path = if book.path.is_absolute() {
            book.path.clone()
        } else {
            spec_base.join(&book.path)
        };
        let source_bytes = fs::read(&source_path)
            .with_context(|| format!("无法读取书籍 {}", source_path.display()))?;
        let source = String::from_utf8(source_bytes.clone())
            .with_context(|| format!("书籍不是有效 UTF-8：{}", source_path.display()))?;
        let prepared = ruby::prepare_text(&source);
        let chars: Vec<char> = prepared.text.chars().collect();
        let paragraph_ranges = paragraph_ranges(&chars);
        let reading_sentence_ranges = reading_sentence_ranges(&chars);
        let segments = segment_prepared_text(&prepared.text);
        let mut clauses = Vec::new();
        for segment in segments
            .into_iter()
            .filter(|segment| segment.kind == ProductionSegmentKind::Content)
        {
            let text: String = chars[segment.char_range.0..segment.char_range.1]
                .iter()
                .collect();
            if text.is_empty() {
                continue;
            }
            let mut morphemes = analyzer.analyze(&text);
            for morpheme in &mut morphemes {
                morpheme.char_range.0 += segment.char_range.0;
                morpheme.char_range.1 += segment.char_range.0;
            }
            let annotation_start = prepared
                .annotations
                .partition_point(|annotation| annotation.char_range.1 <= segment.char_range.0);
            let annotation_end = prepared
                .annotations
                .partition_point(|annotation| annotation.char_range.0 < segment.char_range.1);
            ruby::override_morpheme_readings_with_chars(
                &chars,
                &mut morphemes,
                &prepared.annotations[annotation_start..annotation_end],
            );
            let range = CharRange::new(segment.char_range.0, segment.char_range.1);
            clauses.push(ClauseRecord {
                clause_id: clauses.len(),
                range,
                paragraph_id: containing_range(&paragraph_ranges, range.start),
                reading_sentence_id: containing_range(&reading_sentence_ranges, range.start),
                morphemes,
            });
        }
        let morpheme_count = clauses.iter().map(|clause| clause.morphemes.len()).sum();
        let chunk = BookChunk {
            schema_version: SUBSTRATE_SCHEMA_VERSION.to_string(),
            book_id: book.book_id.clone(),
            source_sha256: sha256_bytes(&source_bytes),
            normalized_text_sha256: sha256_bytes(prepared.text.as_bytes()),
            character_count: chars.len(),
            ruby_annotations: prepared.annotations,
            paragraph_ranges,
            reading_sentence_ranges,
            clauses,
        };
        let chunk_name = format!("{book_index:06}.msgpack");
        let chunk_path = chunk_directory.join(&chunk_name);
        let mut writer = BufWriter::with_capacity(1024 * 1024, File::create(&chunk_path)?);
        rmp_serde::encode::write_named(&mut writer, &chunk)?;
        writer.flush()?;
        let chunk_bytes = fs::metadata(&chunk_path)?.len();
        total_bytes += chunk_bytes;
        if total_bytes > max_bytes {
            bail!(
                "形态素底座超过空间上限：{} > {} 字节",
                total_bytes,
                max_bytes
            );
        }
        total_characters += chunk.character_count;
        total_morphemes += morpheme_count;
        descriptors.push(SubstrateBookDescriptor {
            book_id: book.book_id.clone(),
            source_path: source_path.canonicalize().unwrap_or(source_path),
            source_sha256: chunk.source_sha256.clone(),
            normalized_text_sha256: chunk.normalized_text_sha256.clone(),
            chunk_path: PathBuf::from("books").join(chunk_name),
            chunk_bytes,
            chunk_sha256: sha256_file(&chunk_path)?,
            character_count: chunk.character_count,
            morpheme_count,
            clause_count: chunk.clauses.len(),
        });
    }
    let manifest = SubstrateManifest {
        schema_version: SUBSTRATE_SCHEMA_VERSION.to_string(),
        substrate_id,
        created_at: Utc::now().to_rfc3339(),
        corpus_spec_sha256: spec_hash,
        fingerprint,
        books: descriptors,
        total_characters,
        total_morphemes,
        total_bytes,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    if total_bytes + manifest_bytes.len() as u64 > max_bytes {
        bail!("底座 manifest 使空间超过上限");
    }
    fs::write(staging.join("manifest.json"), manifest_bytes)?;
    Ok(())
}

pub fn read_manifest(directory: &Path) -> Result<SubstrateManifest> {
    let path = directory.join("manifest.json");
    let manifest: SubstrateManifest = serde_json::from_reader(BufReader::new(
        File::open(&path).with_context(|| format!("无法读取 {}", path.display()))?,
    ))?;
    validate_manifest(&path, &manifest.substrate_id)?;
    Ok(manifest)
}

pub fn read_book_chunk(
    directory: &Path,
    descriptor: &SubstrateBookDescriptor,
) -> Result<BookChunk> {
    let path = directory.join(&descriptor.chunk_path);
    if sha256_file(&path)? != descriptor.chunk_sha256 {
        bail!("底座分书块校验失败：{}", path.display());
    }
    Ok(rmp_serde::from_read(BufReader::new(File::open(path)?))?)
}

fn validate_manifest(path: &Path, expected_id: &str) -> Result<()> {
    let manifest: SubstrateManifest = serde_json::from_reader(BufReader::new(File::open(path)?))?;
    if manifest.schema_version != SUBSTRATE_SCHEMA_VERSION
        || manifest.substrate_id != expected_id
        || manifest.fingerprint.boundary_protocol != TEXT_BOUNDARY_PROTOCOL_VERSION
        || manifest.fingerprint.prepare_text_protocol != ruby::PREPARE_TEXT_PROTOCOL_VERSION
        || manifest.fingerprint.morpheme_compatibility_protocol
            != kotoclip_core::pipeline::morpheme::MORPHEME_COMPATIBILITY_VERSION
    {
        bail!("形态素底座协议或身份不兼容：{}", path.display());
    }
    Ok(())
}

fn validate_corpus_spec(spec: &CorpusSpec) -> Result<()> {
    if spec.schema_version != "kotoclip.quality.corpus.v1" || spec.books.is_empty() {
        bail!("语料清单版本错误或没有书籍");
    }
    let mut ids = HashSet::new();
    for book in &spec.books {
        if book.book_id.trim().is_empty() || !ids.insert(book.book_id.as_str()) {
            bail!("书籍 ID 为空或重复：{}", book.book_id);
        }
    }
    Ok(())
}

fn paragraph_ranges(chars: &[char]) -> Vec<CharRange> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < chars.len() {
        if matches!(chars[index], '\n' | '\r') {
            ranges.push(CharRange::new(start, index));
            if chars[index] == '\r' && chars.get(index + 1) == Some(&'\n') {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    if start <= chars.len() {
        ranges.push(CharRange::new(start, chars.len()));
    }
    ranges
}

fn reading_sentence_ranges(chars: &[char]) -> Vec<CharRange> {
    const CLOSERS: &[char] = &['」', '』', '）', '】', '》', '〉', '〕', '］', '”', '’'];
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut terminal_pending = false;
    for (index, character) in chars.iter().copied().enumerate() {
        let terminal = matches!(character, '。' | '！' | '？' | '!' | '?' | '\n');
        if terminal_pending
            && !matches!(character, '。' | '！' | '？' | '!' | '?')
            && !CLOSERS.contains(&character)
        {
            ranges.push(CharRange::new(start, index));
            start = index;
            terminal_pending = false;
        }
        if terminal {
            terminal_pending = true;
        }
    }
    if start < chars.len() {
        ranges.push(CharRange::new(start, chars.len()));
    }
    ranges
}

fn containing_range(ranges: &[CharRange], offset: usize) -> usize {
    ranges
        .iter()
        .position(|range| range.start <= offset && offset <= range.end)
        .unwrap_or_else(|| ranges.len().saturating_sub(1))
}

pub fn gc_substrates(root: &Path, keep_ids: &HashSet<String>, max_bytes: u64) -> Result<u64> {
    if !root.is_dir() {
        return Ok(0);
    }
    let mut entries = Vec::new();
    let mut total = 0_u64;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let size = directory_size(&entry.path())?;
        total += size;
        let modified = entry.metadata()?.modified().unwrap_or(UNIX_EPOCH);
        entries.push((
            modified,
            entry.file_name().to_string_lossy().to_string(),
            entry.path(),
            size,
        ));
    }
    entries.sort_by_key(|entry| entry.0);
    let mut removed = 0_u64;
    for (_, id, path, size) in entries {
        if total <= max_bytes {
            break;
        }
        if keep_ids.contains(&id) {
            continue;
        }
        fs::remove_dir_all(&path)?;
        total -= size;
        removed += size;
    }
    Ok(removed)
}

fn directory_size(path: &Path) -> Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_ranges_keep_closing_quotes_with_terminal() {
        let chars: Vec<char> = "「甲。」「乙？」末尾".chars().collect();
        let ranges = reading_sentence_ranges(&chars);
        let values: Vec<String> = ranges
            .iter()
            .map(|range| chars[range.start..range.end].iter().collect())
            .collect();
        assert_eq!(values, vec!["「甲。」", "「乙？」", "末尾"]);
    }

    #[test]
    fn paragraph_ranges_preserve_empty_lines() {
        let chars: Vec<char> = "甲\r\n\r\n乙".chars().collect();
        assert_eq!(
            paragraph_ranges(&chars),
            vec![
                CharRange::new(0, 1),
                CharRange::new(3, 3),
                CharRange::new(5, 6)
            ]
        );
    }

    #[test]
    fn root_capacity_keeps_current_substrate_and_collects_old_versions() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("current");
        let old = temp.path().join("old");
        fs::create_dir(&current).unwrap();
        fs::create_dir(&old).unwrap();
        fs::write(current.join("chunk"), [0_u8; 8]).unwrap();
        fs::write(old.join("chunk"), [0_u8; 8]).unwrap();
        enforce_substrate_root_capacity(temp.path(), "current", 10).unwrap();
        assert!(current.is_dir());
        assert!(!old.exists());
    }
}
