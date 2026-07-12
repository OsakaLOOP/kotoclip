pub mod exposure;
pub mod expressions;
pub mod dictionary;
pub mod kanji;
pub mod scoring;
pub mod segmentation;

use rusqlite::Connection;
use std::path::Path;
use std::collections::HashMap;
use std::sync::Mutex;

/// 用户画像引擎，掌管词汇曝光、已知/未知状态以及汉字音训掌握度推导
pub struct ProfileEngine {
    conn: Connection,
    dictionary_choice_cache: Mutex<HashMap<String, String>>,
}

impl ProfileEngine {
    /// 构造函数：初始化本地用户画像 SQLite 数据库，并建立相应表结构与联合唯一索引
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::open(db_path)?;

        // 使用 execute_batch 批量初始化用户画像数据表与索引，避免单条 execute 触发 MultipleStatement 限制
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS exposure_history (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                base_form      TEXT NOT NULL,
                reading        TEXT,
                pos            TEXT,
                exposure_count INTEGER DEFAULT 0,
                last_seen_at   TEXT,
                is_known       BOOLEAN DEFAULT FALSE,
                UNIQUE(base_form, reading)
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_exposure_base_reading 
            ON exposure_history(base_form, reading);

            CREATE TABLE IF NOT EXISTS kanji_knowledge (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                kanji     TEXT NOT NULL,
                reading   TEXT NOT NULL,
                confidence REAL DEFAULT 0.0,
                source_words TEXT,
                UNIQUE(kanji, reading)
            );

            CREATE TABLE IF NOT EXISTS user_merge_rules (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                phrase    TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS user_expression_rules (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                label        TEXT NOT NULL,
                description  TEXT NOT NULL DEFAULT '',
                origin       TEXT NOT NULL DEFAULT 'custom',
                pattern_json TEXT NOT NULL UNIQUE,
                created_at   TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS user_segmentation_choices (
                surface          TEXT PRIMARY KEY,
                morphemes_json   TEXT NOT NULL,
                selected_cost    INTEGER NOT NULL,
                selected_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS user_dictionary_choices (
                query_key       TEXT PRIMARY KEY,
                selected_target TEXT NOT NULL,
                selected_at     TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_kanji_knowledge_char 
            ON kanji_knowledge(kanji);
        ",
        )?;

        // 兼容功能开发期间已经创建的本地表达表。
        let _ = conn.execute(
            "ALTER TABLE user_expression_rules ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE user_expression_rules ADD COLUMN origin TEXT NOT NULL DEFAULT 'custom'",
            [],
        );

        let dictionary_choice_cache = {
            let mut statement =
                conn.prepare("SELECT query_key, selected_target FROM user_dictionary_choices")?;
            let choices = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .flatten()
                .collect();
            choices
        };

        Ok(Self { conn, dictionary_choice_cache: Mutex::new(dictionary_choice_cache) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AnnotatedToken, Bunsetsu, HeadWord, PosTag};

    #[test]
    fn test_kanji_inference_and_scoring() {
        // 创建内存数据库做单元隔离测试
        let engine = ProfileEngine::new(":memory:").expect("无法初始化内存用户数据库");

        // 1. 用户学过 "剣道" (ケンドウ) 并标记已知 (通过对半分，得到 剣->ケン 和 道->ドウ)
        engine.mark_known("剣道", "ケンドウ").expect("标记已知失败");
        // 2. 用户学过单字词 "術" (ジュツ) 并标记已知 (精确绑定，得到 術->ジュツ)
        engine.mark_known("術", "ジュツ").expect("标记已知失败");

        // 3. 此时推断单字掌握度
        let conf_ken = engine.get_kanji_confidence('剣', "ケン").expect("查询失败");
        let conf_jutsu = engine
            .get_kanji_confidence('術', "ジュツ")
            .expect("查询失败");
        assert!(conf_ken > 0.7, "剣 读 ケン 置信度应大于 0.7");
        assert!(conf_jutsu > 0.7, "術 读 ジュツ 置信度应大于 0.7");

        // 4. 出现全新词 "剣術" (ケンジュツ)。在 exposure_history 中没有任何记录。
        // 我们构建一个 AnnotatedToken 模拟 Pipeline 的输出
        let mock_token = AnnotatedToken {
            bunsetsu: Bunsetsu {
                morphemes: Vec::new(),
                surface: "剣術".to_string(),
                head_word: HeadWord {
                    surface: "剣術".to_string(),
                    base_form: "剣術".to_string(),
                    reading: "ケンジュツ".to_string(),
                    pos: PosTag {
                        major: "名詞".to_string(),
                        sub1: "一般".to_string(),
                        sub2: "*".to_string(),
                        sub3: "*".to_string(),
                    },
                },
                grammar_tags: Vec::new(),
                word_formations: Vec::new(),
                function: None,
                char_range: (0, 2),
            },
            novelty_score: 1.0,
            is_selected: false,
            is_known: false,
            inference_reason: None,
            expressions: Vec::new(),
            display_class: "content".to_string(),
        };

        // 5. 对 Token 进行标注评分
        let annotated = engine
            .annotate_tokens(vec![mock_token])
            .expect("评分标注失败");

        let token = &annotated[0];

        // 6. 验证推导结果
        assert!(
            token.novelty_score < 0.7,
            "因为掌握了 剣(ケン) 和 術(ジュツ)，生词评分应该被自动拉低"
        );
        assert!(token.inference_reason.is_some(), "应当给出掌握推导的原因");

        let reason = token.inference_reason.as_ref().unwrap();
        assert!(
            reason.contains("剣") && reason.contains("術"),
            "推导原因应当明确包含汉字"
        );
    }

    #[test]
    fn test_exposure_recording_reduces_novelty_on_next_analysis() {
        let engine = ProfileEngine::new(":memory:").expect("无法初始化内存用户数据库");
        let mock_token = AnnotatedToken {
            bunsetsu: Bunsetsu {
                morphemes: Vec::new(),
                surface: "難語".to_string(),
                head_word: HeadWord {
                    surface: "難語".to_string(),
                    base_form: "難語".to_string(),
                    reading: "ナンゴ".to_string(),
                    pos: PosTag {
                        major: "名詞".to_string(),
                        sub1: "一般".to_string(),
                        sub2: "*".to_string(),
                        sub3: "*".to_string(),
                    },
                },
                grammar_tags: Vec::new(),
                word_formations: Vec::new(),
                function: None,
                char_range: (0, 2),
            },
            novelty_score: 1.0,
            is_selected: false,
            is_known: false,
            inference_reason: None,
            expressions: Vec::new(),
            display_class: "content".to_string(),
        };

        let first = engine
            .annotate_tokens(vec![mock_token.clone()])
            .expect("首次评分失败");
        engine.record_token_exposures(&first).expect("记录曝光失败");
        let second = engine
            .annotate_tokens(vec![mock_token])
            .expect("再次评分失败");

        assert!(second[0].novelty_score < first[0].novelty_score);
        let exposure = engine
            .get_exposure("難語", "ナンゴ")
            .expect("读取曝光失败")
            .expect("曝光记录不存在");
        assert_eq!(exposure.exposure_count, 1);

        let mut progress = Vec::new();
        engine
            .record_token_exposures_with_progress(
                &[first[0].clone(), first[0].clone()],
                |completed, total| {
                    progress.push((completed, total));
                },
            )
            .expect("批量记录曝光失败");
        let exposure = engine
            .get_exposure("難語", "ナンゴ")
            .expect("读取批量曝光失败")
            .expect("批量曝光记录不存在");
        assert_eq!(exposure.exposure_count, 3, "聚合写入必须保留每次词汇出现");
        assert_eq!(progress.last(), Some(&(1, 1)), "进度应按唯一写入项报告");
    }
}
