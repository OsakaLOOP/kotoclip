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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_lexical_rules_are_disabled_for_review() {
        let json = r#"{
            "parts": [],
            "expression_type": "lexical_unit",
            "boundary_effect": "merge_lexical_unit"
        }"#;
        let parsed = parse_pattern_json(json).unwrap();
        assert!(!parsed.enabled);
        assert!(parsed.requires_review);
        assert_eq!(parsed.schema_version, 1);
    }

    #[test]
    fn user_rule_v2_rejects_unknown_fields() {
        let json = r#"{
            "schema_version": 2,
            "rule_version": 1,
            "enabled": true,
            "requires_review": false,
            "parts": [],
            "expression_type": "idiom",
            "priority": 70,
            "boundary_effect": "annotate_only",
            "unknown": true
        }"#;
        assert!(parse_pattern_json(json).is_err());
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

fn part_match_offset(
    actual: &ExpressionPatternPart,
    expected: &ExpressionPatternPart,
) -> Option<usize> {
    if expected.is_any {
        return Some(0);
    }
    let range_matches = |actual_start: usize, expected_start: usize, len: usize| {
        let actual_end = actual_start + len;
        let expected_end = expected_start + len;
        (expected.pos_details.is_empty()
            || actual.pos_details[actual_start..actual_end]
                == expected.pos_details[expected_start..expected_end])
            && (expected.conjugation_types.is_empty()
                || actual.conjugation_types[actual_start..actual_end]
                    == expected.conjugation_types[expected_start..expected_end])
            && (expected.conjugation_forms.is_empty()
                || actual.conjugation_forms[actual_start..actual_end]
                    == expected.conjugation_forms[expected_start..expected_end])
    };
    match expected.alignment.as_str() {
        "suffix" => {
            if actual.pos.len() < expected.pos.len() {
                return None;
            }
            let off = actual.pos.len() - expected.pos.len();
            (actual.pos[off..] == expected.pos[..]
                && range_matches(off, 0, expected.pos.len())
                && (expected.is_slot || actual.lemmas[off..] == expected.lemmas[..]))
                .then_some(off)
        }
        "prefix" => {
            if actual.pos.len() < expected.pos.len() {
                return None;
            }
            (actual.pos[..expected.pos.len()] == expected.pos[..]
                && range_matches(0, 0, expected.pos.len())
                && (expected.is_slot
                    || actual.lemmas[..expected.lemmas.len()] == expected.lemmas[..]))
                .then_some(0)
        }
        _ => (actual.pos == expected.pos
            && range_matches(0, 0, expected.pos.len())
            && (expected.is_slot || actual.lemmas == expected.lemmas))
            .then_some(0),
    }
}

fn parts_match(actual: &ExpressionPatternPart, expected: &ExpressionPatternPart) -> bool {
    part_match_offset(actual, expected).is_some()
}

fn token_part(token: &AnnotatedToken) -> ExpressionPatternPart {
    let morphemes = token
        .bunsetsu
        .morphemes
        .iter()
        .filter(|morpheme| !morpheme.surface.trim().is_empty());
    let mut lemmas = Vec::new();
    let mut pos = Vec::new();
    let mut pos_details = Vec::new();
    let mut conjugation_types = Vec::new();
    let mut conjugation_forms = Vec::new();
    for morpheme in morphemes {
        if let Some(lemma) =
            structural_lemma(&morpheme.surface, &morpheme.base_form, &morpheme.pos.major)
        {
            lemmas.push(lemma);
            pos.push(morpheme.pos.major.clone());
            pos_details.push(morpheme.pos.clone());
            conjugation_types.push(morpheme.conjugation_type.clone());
            conjugation_forms.push(morpheme.conjugation_form.clone());
        }
    }
    ExpressionPatternPart {
        lemmas,
        pos,
        pos_details,
        conjugation_types,
        conjugation_forms,
        surface_hint: token.bunsetsu.surface.clone(),
        is_slot: false,
        alignment: "full".to_string(),
        is_any: false,
    }
}

fn matched_part_char_range(
    token: &AnnotatedToken,
    expected: &ExpressionPatternPart,
) -> Option<(usize, usize)> {
    let actual = token_part(token);
    let offset = part_match_offset(&actual, expected)?;
    if expected.is_any {
        return Some(token.bunsetsu.char_range);
    }
    let morphemes: Vec<_> = token
        .bunsetsu
        .morphemes
        .iter()
        .filter(|morpheme| {
            !morpheme.surface.trim().is_empty()
                && structural_lemma(&morpheme.surface, &morpheme.base_form, &morpheme.pos.major)
                    .is_some()
        })
        .collect();
    let end = offset + expected.pos.len();
    (end > offset && end <= morphemes.len()).then(|| {
        (
            morphemes[offset].char_range.0,
            morphemes[end - 1].char_range.1,
        )
    })
}

fn token_part_masked(token: &AnnotatedToken, mask: &[bool]) -> ExpressionPatternPart {
    let mut lemmas = Vec::new();
    let mut pos = Vec::new();
    let mut pos_details = Vec::new();
    let mut conjugation_types = Vec::new();
    let mut conjugation_forms = Vec::new();
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
                pos_details.push(morpheme.pos.clone());
                conjugation_types.push(morpheme.conjugation_type.clone());
                conjugation_forms.push(morpheme.conjugation_form.clone());
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
        pos_details,
        conjugation_types,
        conjugation_forms,
        surface_hint: token.bunsetsu.surface.clone(),
        is_slot: false,
        alignment,
        is_any: false,
    }
}

fn canonical_part(part: &ExpressionPatternPart) -> ExpressionPatternPart {
    let mut lemmas = Vec::new();
    let mut pos = Vec::new();
    let mut pos_details = Vec::new();
    let mut conjugation_types = Vec::new();
    let mut conjugation_forms = Vec::new();
    for (index, (lemma, major)) in part.lemmas.iter().zip(&part.pos).enumerate() {
        if let Some(lemma) = structural_lemma(lemma, lemma, major) {
            lemmas.push(lemma);
            pos.push(major.clone());
            if let Some(detail) = part.pos_details.get(index) {
                pos_details.push(detail.clone());
            }
            if let Some(value) = part.conjugation_types.get(index) {
                conjugation_types.push(value.clone());
            }
            if let Some(value) = part.conjugation_forms.get(index) {
                conjugation_forms.push(value.clone());
            }
        }
    }
    ExpressionPatternPart {
        lemmas,
        pos,
        pos_details,
        conjugation_types,
        conjugation_forms,
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

struct ParsedExpressionPattern {
    schema_version: u32,
    rule_version: u32,
    enabled: bool,
    requires_review: bool,
    parts: Vec<ExpressionPatternPart>,
    gap_after: Option<usize>,
    gap_bunsetsu: (usize, usize),
    expression_type: String,
    priority: i32,
    boundary_effect: String,
}

fn parse_pattern_json(json: &str) -> Result<ParsedExpressionPattern, Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Envelope {
        #[serde(default = "default_schema_local")]
        schema_version: u32,
        #[serde(default = "default_rule_version_local")]
        rule_version: u32,
        #[serde(default = "default_enabled_local")]
        enabled: bool,
        #[serde(default)]
        requires_review: bool,
        parts: Vec<ExpressionPatternPart>,
        #[serde(default)]
        gap_after: Option<usize>,
        #[serde(default = "default_gap_range_local")]
        gap_bunsetsu: (usize, usize),
        #[serde(default = "default_kind_local")]
        expression_type: String,
        #[serde(default = "default_priority_local")]
        priority: i32,
        #[serde(default = "default_boundary_effect_local")]
        boundary_effect: String,
    }

    fn default_schema_local() -> u32 {
        1
    }
    fn default_rule_version_local() -> u32 {
        1
    }
    fn default_enabled_local() -> bool {
        true
    }
    fn default_gap_range_local() -> (usize, usize) {
        (0, 10)
    }
    fn default_kind_local() -> String {
        "grammar_construction".to_string()
    }
    fn default_priority_local() -> i32 {
        50
    }
    fn default_boundary_effect_local() -> String {
        "annotate_only".to_string()
    }

    if let Ok(env) = serde_json::from_str::<Envelope>(json) {
        if env.schema_version == 0 || env.schema_version > 2 || env.rule_version == 0 {
            return Err("用户表达规则版本非法".into());
        }
        let migrated_lexical = env.expression_type == "lexical_unit";
        return Ok(ParsedExpressionPattern {
            schema_version: env.schema_version,
            rule_version: env.rule_version,
            enabled: env.enabled && !migrated_lexical,
            requires_review: env.requires_review || migrated_lexical,
            parts: env.parts,
            gap_after: env.gap_after,
            gap_bunsetsu: env.gap_bunsetsu,
            expression_type: env.expression_type,
            priority: env.priority,
            boundary_effect: env.boundary_effect,
        });
    }

    let parts: Vec<ExpressionPatternPart> = serde_json::from_str(json)?;
    Ok(ParsedExpressionPattern {
        schema_version: 1,
        rule_version: 1,
        enabled: true,
        requires_review: false,
        parts,
        gap_after: None,
        gap_bunsetsu: (0, 10),
        expression_type: "grammar_construction".to_string(),
        priority: 50,
        boundary_effect: "annotate_only".to_string(),
    })
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
        expression_type: &str,
        priority: i32,
        boundary_effect: &str,
    ) -> Result<ExpressionRule, Box<dyn std::error::Error>> {
        if tokens.is_empty() {
            return Err("表达规则至少需要一个文节".into());
        }
        if !matches!(
            expression_type,
            "idiom" | "grammar_construction" | "correlative"
        ) {
            return Err("未知或已迁移的表达类型；词汇单位必须进入构词层".into());
        }
        if expression_type == "correlative" && gap_after.is_none() {
            return Err("非连续呼应必须配置前后锚点之间的间隔".into());
        }
        if boundary_effect != "annotate_only" {
            return Err("表达规则只能添加语义注解，不能修改构词或文节边界".into());
        }
        if tokens.iter().any(|token| {
            token.bunsetsu.surface.contains(['\n', '\r']) || token.bunsetsu.morphemes.is_empty()
        }) {
            return Err("跨文节表达不能跨越段落边界或空文节".into());
        }

        let states = if bunsetsu_states.is_empty() {
            vec!["fixed".to_string(); tokens.len()]
        } else {
            if bunsetsu_states.len() != tokens.len() {
                return Err("文节匹配状态数量与所选文节不一致".into());
            }
            bunsetsu_states.to_vec()
        };

        let masks = if morpheme_masks.is_empty() {
            tokens
                .iter()
                .map(|t| vec![true; t.bunsetsu.morphemes.len()])
                .collect::<Vec<_>>()
        } else {
            if morpheme_masks.len() != tokens.len()
                || morpheme_masks
                    .iter()
                    .zip(tokens)
                    .any(|(mask, token)| mask.len() != token.bunsetsu.morphemes.len())
            {
                return Err("语素选择范围与所选文节不一致".into());
            }
            morpheme_masks.to_vec()
        };

        let mut parts = Vec::new();
        let mut first_gap_idx = None;
        let mut gap_count = 0;
        let mut parts_before_gap = 0;

        for (i, state) in states.iter().enumerate() {
            if !matches!(state.as_str(), "fixed" | "slot" | "any" | "gap") {
                return Err(format!("未知的文节匹配状态：{state}").into());
            }
            if state == "gap" {
                if first_gap_idx.is_none() {
                    first_gap_idx = Some(i);
                    parts_before_gap = parts.len();
                }
                gap_count += 1;
            } else {
                if state != "any" {
                    let selected: Vec<_> = masks[i]
                        .iter()
                        .enumerate()
                        .filter_map(|(index, selected)| selected.then_some(index))
                        .collect();
                    if selected.is_empty()
                        || selected.windows(2).any(|window| window[1] != window[0] + 1)
                    {
                        return Err(format!("文节 {} 的规则语素必须连续且非空", i + 1).into());
                    }
                }
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
        if gap_after.is_some_and(|index| index + 1 >= parts.len()) {
            return Err("可变间隔必须位于两个有效锚点之间".into());
        }

        #[derive(Serialize)]
        struct ExpressionPatternEnvelope<'a> {
            schema_version: u32,
            rule_version: u32,
            enabled: bool,
            requires_review: bool,
            parts: &'a [ExpressionPatternPart],
            gap_after: Option<usize>,
            gap_bunsetsu: (usize, usize),
            expression_type: &'a str,
            priority: i32,
            boundary_effect: &'a str,
        }

        let envelope = ExpressionPatternEnvelope {
            schema_version: 2,
            rule_version: 1,
            enabled: true,
            requires_review: false,
            parts: &parts,
            gap_after,
            gap_bunsetsu: (0, 10),
            expression_type,
            priority,
            boundary_effect,
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
                let pattern = parse_pattern_json(&pattern_json)?;
                Ok(ExpressionRule {
                    id,
                    schema_version: pattern.schema_version,
                    rule_version: pattern.rule_version,
                    enabled: pattern.enabled,
                    requires_review: pattern.requires_review,
                    label,
                    description,
                    origin,
                    expression_type: pattern.expression_type,
                    priority: pattern.priority,
                    boundary_effect: pattern.boundary_effect,
                    parts: pattern.parts,
                    gap_after: pattern.gap_after,
                    gap_bunsetsu: pattern.gap_bunsetsu,
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
            let pattern = parse_pattern_json(&pattern_json)?;
            rules.push(ExpressionRule {
                id,
                schema_version: pattern.schema_version,
                rule_version: pattern.rule_version,
                enabled: pattern.enabled,
                requires_review: pattern.requires_review,
                label,
                description,
                origin,
                expression_type: pattern.expression_type,
                priority: pattern.priority,
                boundary_effect: pattern.boundary_effect,
                parts: pattern.parts,
                gap_after: pattern.gap_after,
                gap_bunsetsu: pattern.gap_bunsetsu,
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
            // lexical_unit 已迁移到构词层。旧规则保留只读，但不得再从表达层改写边界。
            if !rule.enabled || rule.requires_review || rule.expression_type == "lexical_unit" {
                continue;
            }
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
                        let orig_h_last = content_indices[h_end - 1];
                        let orig_t_end = content_indices[t_end - 1] + 1; // 半开区间

                        let head_range = (
                            matched_part_char_range(&tokens[orig_h_start], &head[0])
                                .map_or(tokens[orig_h_start].bunsetsu.char_range.0, |range| {
                                    range.0
                                }),
                            matched_part_char_range(&tokens[orig_h_last], &head[head.len() - 1])
                                .map_or(tokens[orig_h_last].bunsetsu.char_range.1, |range| range.1),
                        );
                        let tail_range = (
                            matched_part_char_range(&tokens[orig_t_start], &tail[0])
                                .map_or(tokens[orig_t_start].bunsetsu.char_range.0, |range| {
                                    range.0
                                }),
                            matched_part_char_range(&tokens[orig_t_end - 1], &tail[tail.len() - 1])
                                .map_or(tokens[orig_t_end - 1].bunsetsu.char_range.1, |range| {
                                    range.1
                                }),
                        );

                        let surface: String = tokens[orig_h_start..orig_t_end]
                            .iter()
                            .map(|token| token.bunsetsu.surface.as_str())
                            .collect();
                        let match_id = format!("{}:{}:{}", rule.id, orig_h_start, orig_t_end);
                        let char_range = (head_range.0, tail_range.1);

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
                                expression_type: rule.expression_type.clone(),
                                priority: rule.priority,
                                boundary_effect: rule.boundary_effect.clone(),
                                confidence: 1.0,
                                position: position.to_string(),
                                token_range: (orig_h_start, orig_t_end),
                                char_range,
                                matched_ranges: vec![head_range, tail_range],
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

                    let char_start =
                        matched_part_char_range(&tokens[orig_start], &canonical_rule[0])
                            .map_or(tokens[orig_start].bunsetsu.char_range.0, |range| range.0);
                    let char_end =
                        matched_part_char_range(&tokens[orig_end - 1], &canonical_rule[width - 1])
                            .map_or(tokens[orig_end - 1].bunsetsu.char_range.1, |range| range.1);

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
                            expression_type: rule.expression_type.clone(),
                            priority: rule.priority,
                            boundary_effect: rule.boundary_effect.clone(),
                            confidence: 1.0,
                            position: position.to_string(),
                            token_range: (orig_start, orig_end),
                            char_range,
                            matched_ranges: vec![char_range],
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
