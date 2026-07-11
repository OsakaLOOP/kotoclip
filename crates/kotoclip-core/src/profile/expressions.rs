use super::ProfileEngine;
use crate::models::{
    AnnotatedToken, ExpressionAnnotation, ExpressionPatternPart, ExpressionRule,
};
use rusqlite::{params, OptionalExtension};

fn normalized_lemma(surface: &str, base_form: &str) -> String {
    if base_form.trim().is_empty() || base_form == "*" {
        surface.to_string()
    } else {
        base_form.to_string()
    }
}

fn structural_lemma(surface: &str, base_form: &str, pos: &str) -> Option<String> {
    let lemma = normalized_lemma(surface, base_form);
    if pos == "助詞" && matches!(lemma.as_str(), "か" | "ね" | "よ") {
        return None;
    }
    if pos != "助動詞" {
        return Some(lemma);
    }
    match lemma.as_str() {
        // 时态与礼貌只改变一次出现的活用形，不限定表达模板。
        "た" | "ます" => None,
        "だ" | "です" => Some("<copula>".to_string()),
        _ => Some(lemma),
    }
}

fn parts_match(left: &ExpressionPatternPart, right: &ExpressionPatternPart) -> bool {
    left.pos == right.pos && (right.is_slot || left.lemmas == right.lemmas)
}

fn token_part(token: &AnnotatedToken) -> ExpressionPatternPart {
    let morphemes = token
        .bunsetsu
        .morphemes
        .iter()
        .filter(|morpheme| !morpheme.surface.trim().is_empty());
    let mut lemmas = Vec::new();
    let mut pos = Vec::new();
    for morpheme in morphemes {
        if let Some(lemma) = structural_lemma(
            &morpheme.surface,
            &morpheme.base_form,
            &morpheme.pos.major,
        ) {
            lemmas.push(lemma);
            pos.push(morpheme.pos.major.clone());
        }
    }
    ExpressionPatternPart {
        lemmas,
        pos,
        surface_hint: token.bunsetsu.surface.clone(),
        is_slot: false,
    }
}

fn canonical_part(part: &ExpressionPatternPart) -> ExpressionPatternPart {
    let mut lemmas = Vec::new();
    let mut pos = Vec::new();
    for (lemma, major) in part.lemmas.iter().zip(&part.pos) {
        if let Some(lemma) = structural_lemma(lemma, lemma, major) {
            lemmas.push(lemma);
            pos.push(major.clone());
        }
    }
    ExpressionPatternPart {
        lemmas,
        pos,
        surface_hint: part.surface_hint.clone(),
        is_slot: part.is_slot,
    }
}

fn default_label(tokens: &[AnnotatedToken]) -> String {
    let surface: String = tokens.iter().map(|token| token.bunsetsu.surface.as_str()).collect();
    let mut label: String = surface.chars().take(18).collect();
    if surface.chars().count() > 18 {
        label.push('…');
    }
    label
}

impl ProfileEngine {
    /// 将一段跨文节选择保存为可复用表达规则。规则以辞书形和词性签名匹配，
    /// 不改变原文节边界，也不依赖一次性的活用表层形。
    pub fn add_expression_rule(
        &self,
        tokens: &[AnnotatedToken],
        label: Option<&str>,
        description: Option<&str>,
        slot_indices: &[usize],
    ) -> Result<ExpressionRule, Box<dyn std::error::Error>> {
        if tokens.len() < 2 {
            return Err("跨文节表达至少需要两个文节".into());
        }
        if tokens.iter().any(|token| {
            token.bunsetsu.surface.contains(['\n', '\r'])
                || token.bunsetsu.morphemes.is_empty()
        }) {
            return Err("跨文节表达不能跨越段落边界或空文节".into());
        }

        let mut parts: Vec<ExpressionPatternPart> = tokens.iter().map(token_part).collect();
        for index in slot_indices {
            let Some(part) = parts.get_mut(*index) else {
                return Err(format!("槽位索引 {index} 超出所选文节范围").into());
            };
            part.is_slot = true;
        }
        let pattern_json = serde_json::to_string(&parts)?;
        let label = label
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_label(tokens));
        let description = description.map(str::trim).unwrap_or_default();

        self.conn.execute(
            "INSERT INTO user_expression_rules (label, description, origin, pattern_json)
             VALUES (?1, ?2, 'custom', ?3)
             ON CONFLICT(pattern_json) DO UPDATE SET
                label = excluded.label,
                description = excluded.description",
            params![label, description, pattern_json],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM user_expression_rules WHERE pattern_json = ?1",
            [&pattern_json],
            |row| row.get(0),
        )?;
        self.expression_rule(id)?.ok_or_else(|| "表达规则写入后无法读取".into())
    }

    pub fn expression_rule(
        &self,
        id: i64,
    ) -> Result<Option<ExpressionRule>, Box<dyn std::error::Error>> {
        let row = self.conn.query_row(
            "SELECT id, label, description, origin, pattern_json, created_at
             FROM user_expression_rules WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        ).optional()?;
        row.map(|(id, label, description, origin, pattern_json, created_at)| {
            Ok(ExpressionRule {
                id,
                label,
                description,
                origin,
                parts: serde_json::from_str(&pattern_json)?,
                created_at,
            })
        }).transpose()
    }

    pub fn get_expression_rules(
        &self,
    ) -> Result<Vec<ExpressionRule>, Box<dyn std::error::Error>> {
        let mut statement = self.conn.prepare(
            "SELECT id, label, description, origin, pattern_json, created_at
             FROM user_expression_rules ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut rules = Vec::new();
        for row in rows {
            let (id, label, description, origin, pattern_json, created_at) = row?;
            rules.push(ExpressionRule {
                id,
                label,
                description,
                origin,
                parts: serde_json::from_str(&pattern_json)?,
                created_at,
            });
        }
        Ok(rules)
    }

    pub fn delete_expression_rule(&self, id: i64) -> rusqlite::Result<bool> {
        Ok(self.conn.execute("DELETE FROM user_expression_rules WHERE id = ?1", [id])? > 0)
    }

    /// 在不改变文节结构的前提下附加跨文节表达注解。允许不同规则重叠，
    /// 以便后续交互比较更长句式与其中较短的固定表达。
    pub fn apply_expression_rules(
        &self,
        tokens: &mut [AnnotatedToken],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        for token in tokens.iter_mut() {
            token.expressions.clear();
        }
        let rules = self.get_expression_rules()?;
        let signatures: Vec<ExpressionPatternPart> = tokens.iter().map(token_part).collect();
        let mut matched = 0;

        for rule in rules {
            let canonical_rule: Vec<ExpressionPatternPart> =
                rule.parts.iter().map(canonical_part).collect();
            let width = rule.parts.len();
            if width < 2 || width > signatures.len() {
                continue;
            }
            for start in 0..=signatures.len() - width {
                if !signatures[start..start + width]
                    .iter()
                    .zip(&canonical_rule)
                    .all(|(actual, expected)| parts_match(actual, expected))
                {
                    continue;
                }
                let end = start + width;
                let surface: String = tokens[start..end]
                    .iter()
                    .map(|token| token.bunsetsu.surface.as_str())
                    .collect();
                let match_id = format!("{}:{}:{}", rule.id, start, end);
                let char_range = (
                    tokens[start].bunsetsu.char_range.0,
                    tokens[end - 1].bunsetsu.char_range.1,
                );
                for (offset, token) in tokens[start..end].iter_mut().enumerate() {
                    let position = if width == 1 {
                        "single"
                    } else if offset == 0 {
                        "start"
                    } else if offset + 1 == width {
                        "end"
                    } else {
                        "middle"
                    };
                    token.expressions.push(ExpressionAnnotation {
                        match_id: match_id.clone(),
                        rule_id: rule.id,
                        label: rule.label.clone(),
                        description: rule.description.clone(),
                        origin: rule.origin.clone(),
                        position: position.to_string(),
                        token_range: (start, end),
                        char_range,
                        surface: surface.clone(),
                    });
                }
                matched += 1;
            }
        }
        Ok(matched)
    }
}
