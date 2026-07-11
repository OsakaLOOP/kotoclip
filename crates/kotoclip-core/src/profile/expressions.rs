use super::ProfileEngine;
use crate::models::{AnnotatedToken, ExpressionAnnotation, ExpressionPatternPart, ExpressionRule};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

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
        "だ" | "edit" | "です" => Some("<copula>".to_string()),
        _ => Some(lemma),
    }
}

fn parts_match(actual: &ExpressionPatternPart, expected: &ExpressionPatternPart) -> bool {
    if expected.is_any {
        return true;
    }
    match expected.alignment.as_str() {
        "suffix" => {
            if actual.pos.len() < expected.pos.len() {
                return false;
            }
            let off = actual.pos.len() - expected.pos.len();
            actual.pos[off..] == expected.pos[..]
                && (expected.is_slot || actual.lemmas[off..] == expected.lemmas[..])
        }
        "prefix" => {
            if actual.pos.len() < expected.pos.len() {
                return false;
            }
            actual.pos[..expected.pos.len()] == expected.pos[..]
                && (expected.is_slot
                    || actual.lemmas[..expected.lemmas.len()] == expected.lemmas[..])
        }
        _ => actual.pos == expected.pos && (expected.is_slot || actual.lemmas == expected.lemmas),
    }
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
        if let Some(lemma) =
            structural_lemma(&morpheme.surface, &morpheme.base_form, &morpheme.pos.major)
        {
            lemmas.push(lemma);
            pos.push(morpheme.pos.major.clone());
        }
    }
    ExpressionPatternPart {
        lemmas,
        pos,
        surface_hint: token.bunsetsu.surface.clone(),
        is_slot: false,
        alignment: "full".to_string(),
        is_any: false,
    }
}

fn token_part_masked(token: &AnnotatedToken, mask: &[bool]) -> ExpressionPatternPart {
    let mut lemmas = Vec::new();
    let mut pos = Vec::new();
    let mut selected_indices = Vec::new();
    let mut total_non_empty_count = 0;
    for (idx, morpheme) in token.bunsetsu.morphemes.iter().enumerate() {
        if morpheme.surface.trim().is_empty() {
            continue;
        }
        let included = mask.get(idx).copied().unwrap_or(true);
        if included {
            if let Some(lemma) =
                structural_lemma(&morpheme.surface, &morpheme.base_form, &morpheme.pos.major)
            {
                lemmas.push(lemma);
                pos.push(morpheme.pos.major.clone());
                selected_indices.push(total_non_empty_count);
            }
        }
        total_non_empty_count += 1;
    }

    let alignment = if selected_indices.is_empty() {
        "full".to_string()
    } else {
        let first = selected_indices[0];
        let last = selected_indices[selected_indices.len() - 1];
        let has_left_excluded = first > 0;
        let has_right_excluded = last < total_non_empty_count - 1;

        if has_left_excluded && !has_right_excluded {
            "suffix".to_string()
        } else if !has_left_excluded && has_right_excluded {
            "prefix".to_string()
        } else {
            "full".to_string()
        }
    };

    ExpressionPatternPart {
        lemmas,
        pos,
        surface_hint: token.bunsetsu.surface.clone(),
        is_slot: false,
        alignment,
        is_any: false,
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
        alignment: part.alignment.clone(),
        is_any: part.is_any,
    }
}

fn default_label(tokens: &[AnnotatedToken]) -> String {
    let surface: String = tokens
        .iter()
        .map(|token| token.bunsetsu.surface.as_str())
        .collect();
    let mut label: String = surface.chars().take(18).collect();
    if surface.chars().count() > 18 {
        label.push('…');
    }
    label
}

fn parse_pattern_json(
    json: &str,
) -> Result<(Vec<ExpressionPatternPart>, Option<usize>, (usize, usize)), Box<dyn std::error::Error>>
{
    #[derive(Deserialize)]
    struct Envelope {
        parts: Vec<ExpressionPatternPart>,
        #[serde(default)]
        gap_after: Option<usize>,
        #[serde(default = "default_gap_range_local")]
        gap_bunsetsu: (usize, usize),
    }

    fn default_gap_range_local() -> (usize, usize) {
        (0, 10)
    }

    if let Ok(env) = serde_json::from_str::<Envelope>(json) {
        return Ok((env.parts, env.gap_after, env.gap_bunsetsu));
    }

    let parts: Vec<ExpressionPatternPart> = serde_json::from_str(json)?;
    Ok((parts, None, (0, 10)))
}

impl ProfileEngine {
    /// 将一段跨文节选择保存为可复用表达规则。规则以辞书形和词性签名匹配，
    /// 不改变原文节边界，也不依赖一次性的活用表层形。
    pub fn add_expression_rule(
        &self,
        tokens: &[AnnotatedToken],
        label: Option<&str>,
        description: Option<&str>,
        bunsetsu_states: &[String],
        morpheme_masks: &[Vec<bool>],
        gap_after: Option<usize>,
    ) -> Result<ExpressionRule, Box<dyn std::error::Error>> {
        if tokens.len() < 2 {
            return Err("跨文节表达至少需要两个文节".into());
        }
        if tokens.iter().any(|token| {
            token.bunsetsu.surface.contains(['\n', '\r']) || token.bunsetsu.morphemes.is_empty()
        }) {
            return Err("跨文节表达不能跨越段落边界或空文节".into());
        }

        let states = if bunsetsu_states.is_empty() {
            vec!["fixed".to_string(); tokens.len()]
        } else {
            bunsetsu_states.to_vec()
        };

        let masks = if morpheme_masks.is_empty() {
            tokens
                .iter()
                .map(|t| vec![true; t.bunsetsu.morphemes.len()])
                .collect::<Vec<_>>()
        } else {
            morpheme_masks.to_vec()
        };

        let mut parts = Vec::new();
        let mut first_gap_idx = None;
        let mut gap_count = 0;
        let mut parts_before_gap = 0;

        for (i, state) in states.iter().enumerate() {
            if state == "gap" {
                if first_gap_idx.is_none() {
                    first_gap_idx = Some(i);
                    parts_before_gap = parts.len();
                }
                gap_count += 1;
            } else {
                let mut part = token_part_masked(&tokens[i], &masks[i]);
                if state == "slot" {
                    part.is_slot = true;
                } else if state == "any" {
                    part.is_any = true;
                }
                parts.push(part);
            }
        }

        let gap_after = if gap_after.is_some() {
            gap_after
        } else if gap_count > 0 {
            if parts_before_gap == 0 || parts_before_gap == parts.len() {
                return Err("间隔文节不能位于首尾".into());
            }
            Some(parts_before_gap - 1)
        } else {
            None
        };

        #[derive(Serialize)]
        struct ExpressionPatternEnvelope<'a> {
            parts: &'a [ExpressionPatternPart],
            gap_after: Option<usize>,
            gap_bunsetsu: (usize, usize),
        }

        let envelope = ExpressionPatternEnvelope {
            parts: &parts,
            gap_after,
            gap_bunsetsu: (0, 10),
        };

        let pattern_json = serde_json::to_string(&envelope)?;
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
        self.expression_rule(id)?
            .ok_or_else(|| "表达规则写入后无法读取".into())
    }

    pub fn expression_rule(
        &self,
        id: i64,
    ) -> Result<Option<ExpressionRule>, Box<dyn std::error::Error>> {
        let row = self
            .conn
            .query_row(
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
            )
            .optional()?;
        row.map(
            |(id, label, description, origin, pattern_json, created_at)| {
                let (parts, gap_after, gap_bunsetsu) = parse_pattern_json(&pattern_json)?;
                Ok(ExpressionRule {
                    id,
                    label,
                    description,
                    origin,
                    parts,
                    gap_after,
                    gap_bunsetsu,
                    created_at,
                })
            },
        )
        .transpose()
    }

    pub fn get_expression_rules(&self) -> Result<Vec<ExpressionRule>, Box<dyn std::error::Error>> {
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
            let (parts, gap_after, gap_bunsetsu) = parse_pattern_json(&pattern_json)?;
            rules.push(ExpressionRule {
                id,
                label,
                description,
                origin,
                parts,
                gap_after,
                gap_bunsetsu,
                created_at,
            });
        }
        Ok(rules)
    }

    pub fn delete_expression_rule(&self, id: i64) -> rusqlite::Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM user_expression_rules WHERE id = ?1", [id])?
            > 0)
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

        // 过滤非 content token 并保留索引
        let content_indices: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.display_class == "content")
            .map(|(idx, _)| idx)
            .collect();

        let signatures: Vec<ExpressionPatternPart> = content_indices
            .iter()
            .map(|&idx| token_part(&tokens[idx]))
            .collect();

        let mut matched = 0;

        for rule in rules {
            let canonical_rule: Vec<ExpressionPatternPart> =
                rule.parts.iter().map(canonical_part).collect();

            if let Some(gap_after) = rule.gap_after {
                // ── 呼应匹配 ──
                let head = &canonical_rule[..=gap_after];
                let tail = &canonical_rule[gap_after + 1..];
                let (gap_min, gap_max) = rule.gap_bunsetsu;

                if head.is_empty() || tail.is_empty() || canonical_rule.len() > signatures.len() {
                    continue;
                }

                for h_start in 0..=signatures.len().saturating_sub(head.len()) {
                    if !signatures[h_start..h_start + head.len()]
                        .iter()
                        .zip(head)
                        .all(|(a, e)| parts_match(a, e))
                    {
                        continue;
                    }

                    let h_end = h_start + head.len();

                    for t_start in (h_end + gap_min)..=signatures.len().saturating_sub(tail.len()) {
                        if t_start - h_end > gap_max {
                            break;
                        }

                        let orig_h_end = content_indices[h_end];
                        let orig_t_start = content_indices[t_start];

                        // 句子边界检查：在原始 token 列表中进行，间隔区域内不得出现句号/问号/感叹号/换行
                        let has_boundary = tokens[orig_h_end..orig_t_start].iter().any(|t| {
                            t.display_class == "punctuation"
                                && t.bunsetsu
                                    .surface
                                    .chars()
                                    .any(|c| "。！？！？…".contains(c))
                                || t.display_class == "line_break"
                        });
                        if has_boundary {
                            continue;
                        }

                        if !signatures[t_start..t_start + tail.len()]
                            .iter()
                            .zip(tail)
                            .all(|(a, e)| parts_match(a, e))
                        {
                            continue;
                        }

                        let t_end = t_start + tail.len();

                        let orig_h_start = content_indices[h_start];
                        let orig_t_end = content_indices[t_end - 1] + 1; // 半开区间

                        // 计算精确的字符匹配起点和终点
                        let char_start = {
                            let actual_start = &tokens[orig_h_start];
                            let expected_start = &head[0];
                            let non_empty_morphemes: Vec<_> = actual_start
                                .bunsetsu
                                .morphemes
                                .iter()
                                .filter(|m| !m.surface.trim().is_empty())
                                .collect();
                            if expected_start.alignment == "suffix"
                                && non_empty_morphemes.len() >= expected_start.pos.len()
                            {
                                let idx = non_empty_morphemes.len() - expected_start.pos.len();
                                non_empty_morphemes[idx].char_range.0
                            } else {
                                actual_start.bunsetsu.char_range.0
                            }
                        };

                        let char_end = {
                            let actual_end = &tokens[orig_t_end - 1];
                            let expected_end = &tail[tail.len() - 1];
                            let non_empty_morphemes: Vec<_> = actual_end
                                .bunsetsu
                                .morphemes
                                .iter()
                                .filter(|m| !m.surface.trim().is_empty())
                                .collect();
                            if expected_end.alignment == "prefix"
                                && non_empty_morphemes.len() >= expected_end.pos.len()
                            {
                                let idx = expected_end.pos.len() - 1;
                                non_empty_morphemes[idx].char_range.1
                            } else {
                                actual_end.bunsetsu.char_range.1
                            }
                        };

                        let surface: String = tokens[orig_h_start..orig_t_end]
                            .iter()
                            .map(|token| token.bunsetsu.surface.as_str())
                            .collect();
                        let match_id = format!("{}:{}:{}", rule.id, orig_h_start, orig_t_end);
                        let char_range = (char_start, char_end);

                        let width = orig_t_end - orig_h_start;
                        for (offset, token) in
                            tokens[orig_h_start..orig_t_end].iter_mut().enumerate()
                        {
                            let position = if offset == 0 {
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
                                token_range: (orig_h_start, orig_t_end),
                                char_range,
                                surface: surface.clone(),
                            });
                        }
                        matched += 1;
                        break; // 采取首个贪婪匹配
                    }
                }
            } else {
                // ── 连续匹配 ──
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

                    let orig_start = content_indices[start];
                    let orig_end = content_indices[end - 1] + 1; // 半开区间

                    // 计算精确的字符匹配起点和终点
                    let char_start = {
                        let actual_start = &tokens[orig_start];
                        let expected_start = &canonical_rule[0];
                        let non_empty_morphemes: Vec<_> = actual_start
                            .bunsetsu
                            .morphemes
                            .iter()
                            .filter(|m| !m.surface.trim().is_empty())
                            .collect();
                        if expected_start.alignment == "suffix"
                            && non_empty_morphemes.len() >= expected_start.pos.len()
                        {
                            let idx = non_empty_morphemes.len() - expected_start.pos.len();
                            non_empty_morphemes[idx].char_range.0
                        } else {
                            actual_start.bunsetsu.char_range.0
                        }
                    };

                    let char_end = {
                        let actual_end = &tokens[orig_end - 1];
                        let expected_end = &canonical_rule[width - 1];
                        let non_empty_morphemes: Vec<_> = actual_end
                            .bunsetsu
                            .morphemes
                            .iter()
                            .filter(|m| !m.surface.trim().is_empty())
                            .collect();
                        if expected_end.alignment == "prefix"
                            && non_empty_morphemes.len() >= expected_end.pos.len()
                        {
                            let idx = expected_end.pos.len() - 1;
                            non_empty_morphemes[idx].char_range.1
                        } else {
                            actual_end.bunsetsu.char_range.1
                        }
                    };

                    let surface: String = tokens[orig_start..orig_end]
                        .iter()
                        .map(|token| token.bunsetsu.surface.as_str())
                        .collect();
                    let match_id = format!("{}:{}:{}", rule.id, orig_start, orig_end);
                    let char_range = (char_start, char_end);

                    let orig_width = orig_end - orig_start;
                    for (offset, token) in tokens[orig_start..orig_end].iter_mut().enumerate() {
                        let position = if orig_width == 1 {
                            "single"
                        } else if offset == 0 {
                            "start"
                        } else if offset + 1 == orig_width {
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
                            token_range: (orig_start, orig_end),
                            char_range,
                            surface: surface.clone(),
                        });
                    }
                    matched += 1;
                }
            }
        }
        Ok(matched)
    }
}
