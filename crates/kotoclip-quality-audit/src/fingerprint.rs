use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn sha256_serializable<T: Serialize>(value: &T) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn sha256_tree(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for relative in files {
        let path = root.join(&relative);
        digest.update(relative.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(sha256_file(&path)?.as_bytes());
        digest.update([0]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_files(root: &Path, current: &Path, target: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("无法枚举输入目录 {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, target)?;
        } else if path.is_file() {
            target.push(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ExecutionFingerprint<'a> {
    audit_schema: &'a str,
    cargo_lock_sha256: String,
    core_resources_sha256: String,
    dictionary_tree_sha256: String,
    system_dictionary_sha256: String,
    boundary_protocol: &'a str,
    prepare_text_protocol: &'a str,
    morpheme_compatibility_protocol: &'a str,
    enabled_features: &'a str,
    n_best_in_scope: bool,
}

pub fn execution_fingerprint(
    repository_root: &Path,
    system_dictionary: &Path,
    dictionary_directory: &Path,
) -> Result<String> {
    let fingerprint = ExecutionFingerprint {
        audit_schema: crate::model::AUDIT_SCHEMA_VERSION,
        cargo_lock_sha256: sha256_file(&repository_root.join("Cargo.lock"))?,
        core_resources_sha256: sha256_tree(
            &repository_root.join("crates/kotoclip-core/resources"),
        )?,
        dictionary_tree_sha256: sha256_tree(dictionary_directory)?,
        system_dictionary_sha256: sha256_file(system_dictionary)?,
        boundary_protocol: kotoclip_core::pipeline::TEXT_BOUNDARY_PROTOCOL_VERSION,
        prepare_text_protocol: kotoclip_core::pipeline::ruby::PREPARE_TEXT_PROTOCOL_VERSION,
        morpheme_compatibility_protocol:
            kotoclip_core::pipeline::morpheme::MORPHEME_COMPATIBILITY_VERSION,
        enabled_features: option_env!("CARGO_CFG_FEATURE").unwrap_or(""),
        n_best_in_scope: false,
    };
    sha256_serializable(&fingerprint)
}
