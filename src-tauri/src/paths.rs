use std::path::PathBuf;
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
        let data_dir = if let Ok(env_dir) = std::env::var("KOTOCLIP_DATA_DIR") {
            PathBuf::from(env_dir)
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
                // 在 Tauri 2.0 中，resolve_resource 可以将相对路径解析为实际的安装路径或开发路径
                app.path().resource_dir()?.join("ipadic").join("system.dic")
            }
        };

        let dictionary_dir = data_dir.join("dicts");
        let profile_db = data_dir.join("user_profile.db");
        let grammar_patterns = data_dir.join("grammar_patterns.json");

        // A starter database is optional; user dictionaries are never overwritten.
        std::fs::create_dir_all(&dictionary_dir)?;
        let starter = app.path().resource_dir()?.join("dicts").join("starter.sqlite");
        if starter.exists() {
            let destination = dictionary_dir.join("starter.sqlite");
            if !destination.exists() { std::fs::copy(starter, &destination)?; }
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
