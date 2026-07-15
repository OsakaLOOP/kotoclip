use std::path::PathBuf;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

pub struct AppPaths {
    pub system_dictionary: PathBuf,
    pub dictionary_dir: PathBuf,
    pub profile_db: PathBuf,
    pub data_dir: PathBuf,
    pub grammar_patterns: Option<PathBuf>,
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        // 允许 KOTOCLIP_DATA_DIR 作为显式的开发/测试重写
        let env_data_dir = std::env::var("KOTOCLIP_DATA_DIR").ok();
        // 便携测试包直接把数据放在 EXE 同目录，允许用户双击 GUI 启动。
        let portable_data_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(PathBuf::from))
            .filter(|path| {
                path.join("ipadic").join("system.dic").is_file()
                    && path.join("dicts").is_dir()
            });
        let data_dir = if let Some(env_dir) = &env_data_dir {
            PathBuf::from(env_dir)
        } else if let Some(portable_dir) = &portable_data_dir {
            portable_dir.clone()
        } else {
            app.path().app_data_dir()?
        };

        // system_dictionary 在正常打包运行时，应从打包的只读资源中获取
        // 在开发模式下，也可以作为资源文件解析；如果定义了 KOTOCLIP_DATA_DIR，优先检查该路径下的 ipadic/system.dic
        let mut system_dictionary = None;
        if let Ok(env_dir) = std::env::var("KOTOCLIP_DATA_DIR") {
            let path = PathBuf::from(&env_dir).join("ipadic").join("system.dic");
            if path.exists() {
                system_dictionary = Some(path);
            }
        }

        let system_dictionary = match system_dictionary {
            Some(p) => p,
            None => {
                let portable_dictionary = portable_data_dir
                    .as_ref()
                    .map(|path| path.join("ipadic").join("system.dic"))
                    .filter(|path| path.is_file());
                if let Some(path) = portable_dictionary {
                    path
                } else {
                // 在 Tauri 2.0 中，使用 PathResolver 的 resolve 方法解析资源路径
                    app.path()
                        .resolve("../ipadic/system.dic", BaseDirectory::Resource)
                        .or_else(|_| {
                            app.path()
                                .resolve("ipadic/system.dic", BaseDirectory::Resource)
                        })
                        .or_else(|_| app.path().resolve("system.dic", BaseDirectory::Resource))
                        .unwrap_or_else(|_| {
                            let res_dir = app
                                .path()
                                .resource_dir()
                                .unwrap_or_else(|_| PathBuf::from("."));
                            res_dir.join("ipadic").join("system.dic")
                        })
                }
            }
        };

        // 开发构建直接使用仓库 data/dicts，确保真实外部词典参与流程；
        // 安装构建仍使用应用数据目录，避免依赖源码路径。
        let dictionary_dir = if env_data_dir.is_some() || portable_data_dir.is_some() {
            data_dir.join("dicts")
        } else if cfg!(debug_assertions) {
            let repository_dicts = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("data")
                .join("dicts");
            if repository_dicts.is_dir() {
                repository_dicts
            } else {
                data_dir.join("dicts")
            }
        } else {
            data_dir.join("dicts")
        };
        let profile_db = data_dir.join("user_profile.db");
        let grammar_patterns = data_dir.join("grammar_patterns.json");

        // A starter database is optional; user dictionaries are never overwritten.
        std::fs::create_dir_all(&dictionary_dir)?;
        let starter = app
            .path()
            .resolve("../data/dicts/starter.sqlite", BaseDirectory::Resource)
            .or_else(|_| {
                app.path()
                    .resolve("data/dicts/starter.sqlite", BaseDirectory::Resource)
            })
            .or_else(|_| {
                app.path()
                    .resolve("dicts/starter.sqlite", BaseDirectory::Resource)
            })
            .or_else(|_| {
                app.path()
                    .resolve("starter.sqlite", BaseDirectory::Resource)
            })
            .ok()
            .filter(|p| p.exists());

        if let Some(starter_path) = starter {
            let destination = dictionary_dir.join("starter.sqlite");
            if !destination.exists() {
                std::fs::copy(starter_path, &destination)?;
            }
        }

        Ok(Self {
            system_dictionary,
            dictionary_dir,
            profile_db,
            data_dir,
            grammar_patterns: grammar_patterns.is_file().then_some(grammar_patterns),
        })
    }
}
