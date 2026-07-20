use crate::document::PIPELINE_ARTIFACT_VERSION;
use crate::models::AnnotatedToken;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const CACHE_SCHEMA_VERSION: u32 = 2;
const MAX_CACHE_ENTRIES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLoadPhase {
    Reading,
    Decoding,
    Validating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLoadProgress {
    pub phase: CacheLoadPhase,
    pub completed: usize,
    pub total: usize,
}

#[derive(Debug, Clone)]
pub struct AnalysisCache {
    directory: PathBuf,
    pipeline_fingerprint: String,
}

#[derive(Serialize, Deserialize)]
struct CacheEnvelope {
    schema_version: u32,
    pipeline_artifact_version: u32,
    pipeline_fingerprint: String,
    source_hash: String,
    tokens: Vec<AnnotatedToken>,
}

impl AnalysisCache {
    pub fn new(
        directory: impl Into<PathBuf>,
        system_dictionary: &Path,
        dictionary_directory: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = directory.into();
        std::fs::create_dir_all(&directory)?;
        Ok(Self {
            directory,
            pipeline_fingerprint: pipeline_fingerprint(system_dictionary, dictionary_directory),
        })
    }

    pub fn load(&self, source: &str) -> Option<Vec<AnnotatedToken>> {
        self.load_with_progress(source, |_| {})
    }

    pub fn load_with_progress<F>(&self, source: &str, mut report: F) -> Option<Vec<AnnotatedToken>>
    where
        F: FnMut(CacheLoadProgress),
    {
        let source_hash = source_hash(source);
        let path = self.path_for_hash(&source_hash);
        let metadata = std::fs::metadata(&path).ok()?;
        let total = usize::try_from(metadata.len()).unwrap_or(0);
        report(CacheLoadProgress {
            phase: CacheLoadPhase::Reading,
            completed: 0,
            total,
        });
        let payload = std::fs::read(&path).ok()?;
        report(CacheLoadProgress {
            phase: CacheLoadPhase::Reading,
            completed: payload.len(),
            total: total.max(payload.len()),
        });
        report(CacheLoadProgress {
            phase: CacheLoadPhase::Decoding,
            completed: 0,
            total: 1,
        });
        let envelope: CacheEnvelope = match rmp_serde::from_slice(&payload) {
            Ok(value) => value,
            Err(_) => {
                let _ = std::fs::remove_file(path);
                return None;
            }
        };
        report(CacheLoadProgress {
            phase: CacheLoadPhase::Decoding,
            completed: 1,
            total: 1,
        });
        report(CacheLoadProgress {
            phase: CacheLoadPhase::Validating,
            completed: 0,
            total: 1,
        });
        if envelope.schema_version != CACHE_SCHEMA_VERSION
            || envelope.pipeline_artifact_version != PIPELINE_ARTIFACT_VERSION
            || envelope.pipeline_fingerprint != self.pipeline_fingerprint
            || envelope.source_hash != source_hash
        {
            let _ = std::fs::remove_file(path);
            return None;
        }
        report(CacheLoadProgress {
            phase: CacheLoadPhase::Validating,
            completed: 1,
            total: 1,
        });
        Some(envelope.tokens)
    }

    pub fn store(
        &self,
        source: &str,
        tokens: &[AnnotatedToken],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_hash = source_hash(source);
        let path = self.path_for_hash(&source_hash);
        let temporary = path.with_extension("bin.tmp");
        let envelope = CacheEnvelope {
            schema_version: CACHE_SCHEMA_VERSION,
            pipeline_artifact_version: PIPELINE_ARTIFACT_VERSION,
            pipeline_fingerprint: self.pipeline_fingerprint.clone(),
            source_hash,
            tokens: tokens.to_vec(),
        };
        // 现有 Token schema 含有需由字段名区分的嵌套结构，使用命名 map 保持 serde
        // 语义，仍避免 JSON 的文本编码和转义成本。
        let payload = rmp_serde::to_vec_named(&envelope)?;
        std::fs::write(&temporary, payload)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(temporary, path)?;
        self.prune()?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), Box<dyn std::error::Error>> {
        for entry in std::fs::read_dir(&self.directory)?.flatten() {
            if is_cache_file(&entry.path()) {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    fn path_for_hash(&self, source_hash: &str) -> PathBuf {
        self.directory.join(format!("{source_hash}.bin"))
    }

    fn prune(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut entries: Vec<_> = std::fs::read_dir(&self.directory)?
            .flatten()
            .filter(|entry| is_cache_file(&entry.path()))
            .map(|entry| {
                let modified = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(UNIX_EPOCH);
                (modified, entry.path())
            })
            .collect();
        entries.sort_by_key(|(modified, _)| *modified);
        let remove_count = entries.len().saturating_sub(MAX_CACHE_ENTRIES);
        for (_, path) in entries.into_iter().take(remove_count) {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
}

fn is_cache_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("bin" | "json")
    )
}

fn source_hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

fn pipeline_fingerprint(system_dictionary: &Path, dictionary_directory: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_SCHEMA_VERSION.to_le_bytes());
    hasher.update(PIPELINE_ARTIFACT_VERSION.to_le_bytes());
    for resource in [
        include_bytes!("../resources/word_formation_patterns.json").as_slice(),
        include_bytes!("../resources/lexical_candidate_patterns.json").as_slice(),
        include_bytes!("../resources/bunsetsu_patterns.json").as_slice(),
        include_bytes!("../resources/expression_patterns.json").as_slice(),
        include_bytes!("../resources/grammar/compiled/grammar_catalog.json").as_slice(),
        include_bytes!("../resources/grammar/compiled/grammar_explanations.json").as_slice(),
    ] {
        hasher.update(resource);
    }
    hasher.update(crate::pipeline::grammar::ANALYZER_VERSION.as_bytes());
    hasher.update(crate::pipeline::morphology::ANALYZER_VERSION.as_bytes());
    update_file_signature(&mut hasher, system_dictionary);
    let mut dictionaries: Vec<_> = std::fs::read_dir(dictionary_directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("db" | "sqlite")
            )
        })
        .collect();
    dictionaries.sort();
    for dictionary in dictionaries {
        update_file_signature(&mut hasher, &dictionary);
    }
    format!("{:x}", hasher.finalize())
}

fn update_file_signature(hasher: &mut Sha256, path: &Path) {
    hasher.update(path.to_string_lossy().as_bytes());
    if let Ok(metadata) = path.metadata() {
        hasher.update(metadata.len().to_le_bytes());
        if let Ok(modified) = metadata.modified().and_then(|value| {
            value
                .duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)
        }) {
            hasher.update(modified.as_nanos().to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn corrupted_cache_falls_back_and_is_removed() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-cache-{nonce}"));
        let dictionaries = directory.join("dicts");
        std::fs::create_dir_all(&dictionaries).unwrap();
        let system_dictionary = directory.join("system.dic");
        std::fs::write(&system_dictionary, b"test").unwrap();
        let cache =
            AnalysisCache::new(directory.join("cache"), &system_dictionary, &dictionaries).unwrap();
        let hash = source_hash("本文");
        let path = cache.path_for_hash(&hash);
        std::fs::write(&path, b"broken").unwrap();
        assert!(cache.load("本文").is_none());
        assert!(!path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stale_pipeline_artifact_cache_is_removed() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-cache-artifact-{nonce}"));
        let dictionaries = directory.join("dicts");
        let system_dictionary = directory.join("system.dic");
        std::fs::create_dir_all(&dictionaries).unwrap();
        std::fs::write(&system_dictionary, b"test").unwrap();
        let cache =
            AnalysisCache::new(directory.join("cache"), &system_dictionary, &dictionaries).unwrap();
        let source = "本文";
        let hash = source_hash(source);
        let path = cache.path_for_hash(&hash);
        let stale = CacheEnvelope {
            schema_version: CACHE_SCHEMA_VERSION,
            pipeline_artifact_version: PIPELINE_ARTIFACT_VERSION - 1,
            pipeline_fingerprint: cache.pipeline_fingerprint.clone(),
            source_hash: hash,
            tokens: Vec::new(),
        };
        std::fs::write(&path, rmp_serde::to_vec_named(&stale).unwrap()).unwrap();

        assert!(cache.load(source).is_none());
        assert!(!path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn dictionary_signature_change_invalidates_cached_tokens() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-cache-version-{nonce}"));
        let dictionaries = directory.join("dicts");
        std::fs::create_dir_all(&dictionaries).unwrap();
        let system_dictionary = directory.join("system.dic");
        let dictionary = dictionaries.join("test.sqlite");
        std::fs::write(&system_dictionary, b"system").unwrap();
        std::fs::write(&dictionary, b"v1").unwrap();
        let cache_directory = directory.join("cache");
        let first =
            AnalysisCache::new(&cache_directory, &system_dictionary, &dictionaries).unwrap();
        first.store("本文", &[]).unwrap();
        assert!(first.load("本文").is_some());
        std::fs::write(&dictionary, b"version-two").unwrap();
        let second =
            AnalysisCache::new(&cache_directory, &system_dictionary, &dictionaries).unwrap();
        assert!(second.load("本文").is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cache_capacity_is_bounded() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kotoclip-cache-capacity-{nonce}"));
        let dictionaries = directory.join("dicts");
        std::fs::create_dir_all(&dictionaries).unwrap();
        let system_dictionary = directory.join("system.dic");
        std::fs::write(&system_dictionary, b"system").unwrap();
        let cache_directory = directory.join("cache");
        let cache =
            AnalysisCache::new(&cache_directory, &system_dictionary, &dictionaries).unwrap();
        for index in 0..=MAX_CACHE_ENTRIES {
            cache.store(&format!("本文{index}"), &[]).unwrap();
        }
        let entries = std::fs::read_dir(&cache_directory).unwrap().count();
        assert_eq!(entries, MAX_CACHE_ENTRIES);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
