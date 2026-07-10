use super::ProfileEngine;
use rusqlite::Result;

/// 曝光数据记录
pub struct ExposureRecord {
    pub exposure_count: i32,
    pub is_known: bool,
}

impl ProfileEngine {
    /// 记录一次生词曝光。若单词不存在，则新建记录；若已存在，累加曝光计数并更新时间。
    pub fn record_exposure(&self, base_form: &str, reading: &str, pos: &str) -> Result<()> {
        let sql = "
            INSERT INTO exposure_history (base_form, reading, pos, exposure_count, last_seen_at, is_known)
            VALUES (?1, ?2, ?3, 1, datetime('now'), 0)
            ON CONFLICT(base_form, reading) DO UPDATE SET
                exposure_count = exposure_count + 1,
                last_seen_at = datetime('now')
        ";
        self.conn.execute(sql, [base_form, reading, pos])?;
        Ok(())
    }

    /// 用户主动标记单词为“已知”
    pub fn mark_known(&self, base_form: &str, reading: &str) -> Result<()> {
        let sql = "
            INSERT INTO exposure_history (base_form, reading, exposure_count, last_seen_at, is_known)
            VALUES (?1, ?2, 0, datetime('now'), 1)
            ON CONFLICT(base_form, reading) DO UPDATE SET
                is_known = 1,
                last_seen_at = datetime('now')
        ";
        self.conn.execute(sql, [base_form, reading])?;

        // 标记已知时，同时将此单词所包含的汉字及其读音分解，记录进 kanji_knowledge 供未来推断冷启动
        if let Err(e) = self.update_kanji_knowledge_from_word(base_form, reading) {
            log::error!("更新汉字推导知识库失败 {}+{}: {}", base_form, reading, e);
        }

        Ok(())
    }

    /// 用户主动标记单词为“未知” (撤销已知)
    pub fn mark_unknown(&self, base_form: &str, reading: &str) -> Result<()> {
        let sql = "
            INSERT INTO exposure_history (base_form, reading, is_known, last_seen_at)
            VALUES (?1, ?2, 0, datetime('now'))
            ON CONFLICT(base_form, reading) DO UPDATE SET
                is_known = 0,
                last_seen_at = datetime('now')
        ";
        self.conn.execute(sql, [base_form, reading])?;
        
        // 撤销已知时，也应降低相应汉字的掌握度或做对应清理。
        // 初版暂时静默降低对应汉字置信度。
        if let Err(e) = self.remove_kanji_knowledge_from_word(base_form, reading) {
            log::error!("撤销汉字推导知识库失败 {}+{}: {}", base_form, reading, e);
        }

        Ok(())
    }

    /// 查询特定单词的曝光次数与已知状态
    pub fn get_exposure(&self, base_form: &str, reading: &str) -> Result<Option<ExposureRecord>> {
        let sql = "SELECT exposure_count, is_known FROM exposure_history WHERE base_form = ?1 AND reading = ?2";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query_map([base_form, reading], |row| {
            Ok(ExposureRecord {
                exposure_count: row.get(0)?,
                is_known: row.get(1)?,
            })
        })?;

        if let Some(result) = rows.next() {
            Ok(Some(result?))
        } else {
            Ok(None)
        }
    }

    /// 插入一条强制合并分词的短语规则 (表层形序列，以逗号拼接)
    pub fn add_merge_rule(&self, parts: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        let phrase = parts.join(",");
        let sql = "INSERT OR IGNORE INTO user_merge_rules (phrase) VALUES (?1)";
        self.conn.execute(sql, [&phrase])?;
        Ok(())
    }

    /// 获取所有已登记的用户合并短语规则
    pub fn get_merge_rules(&self) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
        let sql = "SELECT phrase FROM user_merge_rules";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            let phrase: String = row.get(0)?;
            let parts: Vec<String> = phrase.split(',').map(|s| s.to_string()).collect();
            Ok(parts)
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}
