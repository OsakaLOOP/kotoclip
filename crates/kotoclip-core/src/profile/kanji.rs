use super::ProfileEngine;
use rusqlite::Result;
use serde_json;
use std::collections::HashMap;

/// 判断字符是否为汉字 (CJK 统一汉字区间)
fn is_kanji(c: char) -> bool {
    ('\u{4e00}'..='\u{9faf}').contains(&c)
}

/// 提取字符串中的所有汉字
fn extract_kanji(s: &str) -> Vec<char> {
    s.chars().filter(|&c| is_kanji(c)).collect()
}

/// 判断片假名/平假名是否为小写的拗音、促音或辅助字符
fn is_small_kana(c: char) -> bool {
    matches!(
        c,
        'ゃ' | 'ゅ'
            | 'ょ'
            | 'っ'
            | 'ぁ'
            | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | 'ッ'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
    )
}

/// 将读音划分成“音拍 (Morae)”列表，合并拗音如 "ジュ" 和促音 "ッ"
fn split_reading_to_morae(reading: &str) -> Vec<String> {
    let chars: Vec<char> = reading.chars().collect();
    let mut morae = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut s = chars[i].to_string();
        // 如果下一个字符是小写假名，将其合并进当前的音拍中 (如 "ジ" + "ュ" -> "ジュ")
        if i + 1 < chars.len() && is_small_kana(chars[i + 1]) {
            s.push(chars[i + 1]);
            i += 2;
        } else {
            i += 1;
        }
        morae.push(s);
    }
    morae
}

impl ProfileEngine {
    /// 读取当前画像中的全部汉字读音置信度，供单次文章评分复用。
    /// 画像规模通常远小于文本 token 数，避免为每个重复词汇执行多次 SQLite 查询。
    pub(crate) fn get_all_kanji_confidences(&self) -> Result<HashMap<char, HashMap<String, f32>>> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT kanji, reading, confidence FROM kanji_knowledge")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f32>(2)?,
            ))
        })?;
        let mut confidences = HashMap::new();
        for (kanji, reading, confidence) in rows.flatten() {
            if let Some(character) = kanji.chars().next() {
                confidences
                    .entry(character)
                    .or_insert_with(HashMap::new)
                    .insert(reading, confidence);
            }
        }
        Ok(confidences)
    }

    pub(crate) fn infer_novelty_confidence_from_cache(
        &self,
        word: &str,
        reading: &str,
        confidences: &HashMap<char, HashMap<String, f32>>,
    ) -> Result<(f32, Option<String>)> {
        infer_novelty_confidence_with(word, reading, |kanji, kana| {
            Ok(confidences
                .get(&kanji)
                .and_then(|readings| readings.get(kana))
                .copied()
                .unwrap_or(0.0))
        })
    }

    /// 汉字读音学习度更新逻辑。当用户学过一个词后，将其中的汉字和对应假名对齐并存入数据库。
    pub fn update_kanji_knowledge_from_word(&self, word: &str, reading: &str) -> Result<()> {
        let kanjis = extract_kanji(word);
        if kanjis.is_empty() {
            return Ok(());
        }

        let morae = split_reading_to_morae(reading);

        // 1. 若为单汉字词 (例如 "術" -> "ジュツ")，直接进行精准绑定 (置信度为 1.0)
        if kanjis.len() == 1 && word.chars().count() == 1 {
            self.insert_kanji_record(kanjis[0], reading, 1.0, word)?;
            return Ok(());
        }

        // 2. 双字汉字词且读音为 4 个音拍 (如 "剣道" -> "ケンドウ", "剣術" -> "ケンジュツ")
        // 每个汉字分配 2 个音拍 (如 剣->ケン, 道->ドウ 或 剣->ケン, 術->ジュツ)
        if kanjis.len() == 2 && word.chars().all(is_kanji) && morae.len() == 4 {
            let r0 = morae[0..2].join("");
            let r1 = morae[2..4].join("");

            self.insert_kanji_record(kanjis[0], &r0, 0.8, word)?;
            self.insert_kanji_record(kanjis[1], &r1, 0.8, word)?;
            return Ok(());
        }

        // 3. 三字汉字词且读音为 6 个音拍 (如 "美術家" -> "ビジュツカ")
        // 每个汉字分配 2 个音拍 (如 美->ビ, 術->ジュツ, 家->カ)
        if kanjis.len() == 3 && word.chars().all(is_kanji) && morae.len() == 6 {
            let r0 = morae[0..2].join("");
            let r1 = morae[2..4].join("");
            let r2 = morae[4..6].join("");

            self.insert_kanji_record(kanjis[0], &r0, 0.8, word)?;
            self.insert_kanji_record(kanjis[1], &r1, 0.8, word)?;
            self.insert_kanji_record(kanjis[2], &r2, 0.8, word)?;
            return Ok(());
        }

        Ok(())
    }

    /// 用户取消单词“已知”时，清理对应的汉字置信度
    pub fn remove_kanji_knowledge_from_word(&self, word: &str, reading: &str) -> Result<()> {
        let kanjis = extract_kanji(word);
        if kanjis.is_empty() {
            return Ok(());
        }

        let morae = split_reading_to_morae(reading);

        if kanjis.len() == 1 && word.chars().count() == 1 {
            self.decrease_kanji_record(kanjis[0], reading, word)?;
        } else if kanjis.len() == 2 && word.chars().all(is_kanji) && morae.len() == 4 {
            let r0 = morae[0..2].join("");
            let r1 = morae[2..4].join("");
            self.decrease_kanji_record(kanjis[0], &r0, word)?;
            self.decrease_kanji_record(kanjis[1], &r1, word)?;
        } else if kanjis.len() == 3 && word.chars().all(is_kanji) && morae.len() == 6 {
            let r0 = morae[0..2].join("");
            let r1 = morae[2..4].join("");
            let r2 = morae[4..6].join("");
            self.decrease_kanji_record(kanjis[0], &r0, word)?;
            self.decrease_kanji_record(kanjis[1], &r1, word)?;
            self.decrease_kanji_record(kanjis[2], &r2, word)?;
        }

        Ok(())
    }

    /// 基于已知汉字推断生词的“熟练置信度”
    /// 返回值：0.0 (完全未知) 到 1.0 (完全掌握) 之间的置信度
    pub fn infer_novelty_confidence(
        &self,
        word: &str,
        reading: &str,
    ) -> Result<(f32, Option<String>)> {
        infer_novelty_confidence_with(word, reading, |kanji, kana| {
            self.get_kanji_confidence(kanji, kana)
        })
    }

    // 内部函数：写入/更新单汉字的读音与掌握置信度
    fn insert_kanji_record(
        &self,
        kanji: char,
        reading: &str,
        confidence: f32,
        source_word: &str,
    ) -> Result<()> {
        let kanji_str = kanji.to_string();

        let select_sql = "SELECT confidence, source_words FROM kanji_knowledge WHERE kanji = ?1 AND reading = ?2";
        let mut stmt = self.conn.prepare(select_sql)?;
        let mut rows = stmt.query_map([&kanji_str, reading], |row| {
            let conf: f32 = row.get(0)?;
            let src: String = row.get(1)?;
            Ok((conf, src))
        })?;

        if let Some(row) = rows.next() {
            let (existing_conf, existing_src) = row?;
            let mut words: Vec<String> = serde_json::from_str(&existing_src).unwrap_or_default();
            if !words.contains(&source_word.to_string()) {
                words.push(source_word.to_string());
            }
            let new_src = serde_json::to_string(&words).unwrap_or_default();
            let new_conf = (existing_conf + confidence).min(1.0);

            let update_sql = "UPDATE kanji_knowledge SET confidence = ?1, source_words = ?2 WHERE kanji = ?3 AND reading = ?4";
            self.conn.execute(
                update_sql,
                rusqlite::params![new_conf, new_src, &kanji_str, reading],
            )?;
        } else {
            let words = vec![source_word.to_string()];
            let src_json = serde_json::to_string(&words).unwrap_or_default();
            let insert_sql = "INSERT INTO kanji_knowledge (kanji, reading, confidence, source_words) VALUES (?1, ?2, ?3, ?4)";
            self.conn.execute(
                insert_sql,
                rusqlite::params![&kanji_str, reading, confidence, src_json],
            )?;
        }

        Ok(())
    }

    // 内部函数：用户撤销已知时降低汉字掌握置信度
    fn decrease_kanji_record(&self, kanji: char, reading: &str, source_word: &str) -> Result<()> {
        let kanji_str = kanji.to_string();
        let select_sql = "SELECT confidence, source_words FROM kanji_knowledge WHERE kanji = ?1 AND reading = ?2";
        let mut stmt = self.conn.prepare(select_sql)?;
        let mut rows = stmt.query_map([&kanji_str, reading], |row| {
            let conf: f32 = row.get(0)?;
            let src: String = row.get(1)?;
            Ok((conf, src))
        })?;

        if let Some(row) = rows.next() {
            let (existing_conf, existing_src) = row?;
            let mut words: Vec<String> = serde_json::from_str(&existing_src).unwrap_or_default();
            if words.contains(&source_word.to_string()) {
                words.retain(|w| w != source_word);
            }
            let new_src = serde_json::to_string(&words).unwrap_or_default();

            let new_conf = if words.is_empty() {
                0.0
            } else {
                (existing_conf - 0.2).max(0.0)
            };

            let update_sql = "UPDATE kanji_knowledge SET confidence = ?1, source_words = ?2 WHERE kanji = ?3 AND reading = ?4";
            self.conn.execute(
                update_sql,
                rusqlite::params![new_conf, new_src, &kanji_str, reading],
            )?;
        }
        Ok(())
    }

    // 内部函数：获取单字掌握置信度
    pub(crate) fn get_kanji_confidence(&self, kanji: char, reading: &str) -> Result<f32> {
        let kanji_str = kanji.to_string();
        let sql = "SELECT confidence FROM kanji_knowledge WHERE kanji = ?1 AND reading = ?2";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query_map([&kanji_str, reading], |row| {
            let conf: f32 = row.get(0)?;
            Ok(conf)
        })?;

        if let Some(row) = rows.next() {
            Ok(row?)
        } else {
            Ok(0.0)
        }
    }
}

fn infer_novelty_confidence_with<F>(
    word: &str,
    reading: &str,
    mut confidence_for: F,
) -> Result<(f32, Option<String>)>
where
    F: FnMut(char, &str) -> Result<f32>,
{
    let kanjis = extract_kanji(word);
    if kanjis.is_empty() {
        return Ok((0.0, None));
    }
    let morae = split_reading_to_morae(reading);
    if kanjis.len() == 2 && word.chars().all(is_kanji) && morae.len() == 4 {
        let r0 = morae[0..2].join("");
        let r1 = morae[2..4].join("");
        let conf0 = confidence_for(kanjis[0], &r0)?;
        let conf1 = confidence_for(kanjis[1], &r1)?;
        if conf0 > 0.1 && conf1 > 0.1 {
            return Ok((
                conf0.min(conf1),
                Some(format!(
                    "根据已掌握汉字及其音读推断：'{}'({}) 与 '{}'({}) 已知",
                    kanjis[0], r0, kanjis[1], r1
                )),
            ));
        }
    }
    if kanjis.len() == 3 && word.chars().all(is_kanji) && morae.len() == 6 {
        let r0 = morae[0..2].join("");
        let r1 = morae[2..4].join("");
        let r2 = morae[4..6].join("");
        let conf0 = confidence_for(kanjis[0], &r0)?;
        let conf1 = confidence_for(kanjis[1], &r1)?;
        let conf2 = confidence_for(kanjis[2], &r2)?;
        if conf0 > 0.1 && conf1 > 0.1 && conf2 > 0.1 {
            return Ok((
                conf0.min(conf1).min(conf2),
                Some(format!(
                    "根据已掌握汉字及其音读推断：'{}'({}), '{}'({}) 与 '{}'({}) 已知",
                    kanjis[0], r0, kanjis[1], r1, kanjis[2], r2
                )),
            ));
        }
    }
    Ok((0.0, None))
}
