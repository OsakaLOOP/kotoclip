use std::path::{Path, PathBuf};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

pub struct AppPaths {
    pub system_dictionary: PathBuf,
    pub dictionary_source_dir: PathBuf,
    pub dictionary_dir: PathBuf,
    pub profile_db: PathBuf,
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let env_data_dir = std::env::var("KOTOCLIP_DATA_DIR").ok().map(PathBuf::from);
        let portable_root = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(PathBuf::from))
            .filter(|path| {
                path.join("ipadic").join("system.dic").is_file()
                    && path.join("dict-sources").is_dir()
            });
        let data_dir = env_data_dir
            .clone()
            .unwrap_or(app.path().app_data_dir()?);

        let system_dictionary = env_data_dir
            .as_ref()
            .map(|path| path.join("ipadic").join("system.dic"))
            .filter(|path| path.is_file())
            .or_else(|| {
                portable_root
                    .as_ref()
                    .map(|path| path.join("ipadic").join("system.dic"))
                    .filter(|path| path.is_file())
            })
            .or_else(|| {
                let repository_dictionary = repository_root.join("ipadic").join("system.dic");
                (cfg!(debug_assertions) && repository_dictionary.is_file())
                    .then_some(repository_dictionary)
            })
            .or_else(|| {
                [
                    "../ipadic/system.dic",
                    "ipadic/system.dic",
                    "system.dic",
                ]
                .into_iter()
                .find_map(|candidate| {
                    app.path()
                        .resolve(candidate, BaseDirectory::Resource)
                        .ok()
                        .filter(|path| path.is_file())
                })
            })
            .ok_or("未找到 ipadic/system.dic")?;

        let dictionary_source_dir = if let Some(path) = &env_data_dir {
            path.join("dict-sources")
        } else if let Some(path) = &portable_root {
            path.join("dict-sources")
        } else if cfg!(debug_assertions) && repository_root.join("data/dict-sources").is_dir() {
            repository_root.join("data/dict-sources")
        } else {
            let resource_source = [
                "../data/dict-sources/starter.kdict",
                "data/dict-sources/starter.kdict",
                "dict-sources/starter.kdict",
                "starter.kdict",
            ]
            .into_iter()
            .find_map(|candidate| {
                app.path()
                    .resolve(candidate, BaseDirectory::Resource)
                    .ok()
                    .filter(|path| path.is_file())
            });
            if let Some(source) = resource_source {
                source
                    .parent()
                    .map(PathBuf::from)
                    .ok_or("starter.kdict 缺少父目录")?
            } else {
                data_dir.join("dict-sources")
            }
        };

        let dictionary_dir = if env_data_dir.is_some() {
            data_dir.join("dicts")
        } else if cfg!(debug_assertions) {
            repository_root.join("data/dicts")
        } else {
            data_dir.join("dicts")
        };
        std::fs::create_dir_all(&dictionary_source_dir)?;
        std::fs::create_dir_all(&dictionary_dir)?;

        let profile_db = data_dir.join("user_profile.db");
        Ok(Self {
            system_dictionary,
            dictionary_source_dir,
            dictionary_dir,
            profile_db,
            data_dir,
        })
    }
}
