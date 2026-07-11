use super::ProfileEngine;
use crate::models::AnnotatedToken;
use crate::performance::TimingCollector;
use rusqlite::{params, Result};
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

/// 曝光数据记录
#[derive(Clone)]
pub struct ExposureRecord {
    pub exposure_count: i32,
    pub is_known: bool,
}

impl ProfileEngine {
    /// Record one exposure for every lexical token that was actually presented.
    pub fn record_token_exposures(&self, tokens: &[AnnotatedToken]) -> Result<()> {
        self.record_token_exposures_with_progress(tokens, |_, _| {})
    }

    pub fn record_token_exposures_with_progress<F>(
        &self,
        tokens: &[AnnotatedToken],
        mut report: F,
    ) -> Result<()>
    where
        F: FnMut(usize, usize),
    {
        // 同一页面常重复出现相同词汇。先聚合出现次数，再在一个事务中写入，
        // 保持曝光计数语义不变，同时避免每个 token 触发一次磁盘提交。
        let mut exposures: BTreeMap<(String, String, String), i64> = BTreeMap::new();
        for token in tokens {
            let head = &token.bunsetsu.head_word;
            if head.pos.major != "記号" && !head.base_form.trim().is_empty() {
                *exposures
                    .entry((
                        head.base_form.clone(),
                        head.reading.clone(),
                        head.pos.major.clone(),
                    ))
                    .or_insert(0) += 1;
            }
        }

        let total = exposures.len();
        let report_step = (total / 100).max(1);
        report(0, total);

        let transaction = self.conn.unchecked_transaction()?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO exposure_history
                    (base_form, reading, pos, exposure_count, last_seen_at, is_known)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'), 0)
                 ON CONFLICT(base_form, reading) DO UPDATE SET
                    exposure_count = exposure_count + excluded.exposure_count,
                    last_seen_at = datetime('now')",
            )?;
            for (index, ((base_form, reading, pos), count)) in exposures.into_iter().enumerate() {
                statement.execute(params![base_form, reading, pos, count])?;
                let completed = index + 1;
                if completed == total || completed % report_step == 0 {
                    report(completed, total);
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// 性能诊断版曝光记录，将内存聚合和 SQLite 写入分开计时。
    pub fn record_token_exposures_profiled(
        &self,
        tokens: &[AnnotatedToken],
        timings: &mut TimingCollector,
    ) -> Result<()> {
        let aggregation_started = Instant::now();
        let mut exposures: BTreeMap<(String, String, String), i64> = BTreeMap::new();
        for token in tokens {
            let head = &token.bunsetsu.head_word;
            if head.pos.major != "記号" && !head.base_form.trim().is_empty() {
                *exposures
                    .entry((
                        head.base_form.clone(),
                        head.reading.clone(),
                        head.pos.major.clone(),
                    ))
                    .or_insert(0) += 1;
            }
        }
        timings.add("曝光聚合", aggregation_started.elapsed());

        let writing_started = Instant::now();
        let transaction = self.conn.unchecked_transaction()?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO exposure_history
                    (base_form, reading, pos, exposure_count, last_seen_at, is_known)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'), 0)
                 ON CONFLICT(base_form, reading) DO UPDATE SET
                    exposure_count = exposure_count + excluded.exposure_count,
                    last_seen_at = datetime('now')",
            )?;
            for ((base_form, reading, pos), count) in exposures {
                statement.execute(params![base_form, reading, pos, count])?;
            }
        }
        transaction.commit()?;
        timings.add("曝光 SQLite 写入", writing_started.elapsed());
        Ok(())
    }

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

    /// 一次读取评分所需的全部曝光记录，避免章节内逐 token 查询 SQLite。
    pub fn get_all_exposures(&self) -> Result<HashMap<String, HashMap<String, ExposureRecord>>> {
        let mut statement = self.conn.prepare_cached(
            "SELECT base_form, COALESCE(reading, ''), exposure_count, is_known FROM exposure_history",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                ExposureRecord {
                    exposure_count: row.get(2)?,
                    is_known: row.get(3)?,
                },
            ))
        })?;
        let mut exposures: HashMap<String, HashMap<String, ExposureRecord>> = HashMap::new();
        for ((base_form, reading), record) in rows.flatten() {
            exposures.entry(base_form).or_default().insert(reading, record);
        }
        Ok(exposures)
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
