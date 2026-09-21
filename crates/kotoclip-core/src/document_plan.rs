//! 文档坐标、分析窗口与出现锚点。规划阶段处理全文，模型按窗口执行。
use kotoclip_nlp::{external::text_digest, model::RegisterRouting, prepare::{prepare_text, PreparationMap, PreparedText}, routing::{self, RegisterPolicy}};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

pub const VERSION: &str = "kotoclip.document-plan.v1";
pub const UNIT_CHARACTERS: usize = 768;
pub const CONTEXT_CHARACTERS: usize = 192;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OccurrenceAnchor {
    pub document_id: String,
    pub text_version: String,
    pub char_range: [usize; 2],
    pub text_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisUnit {
    pub id: String,
    pub anchor: OccurrenceAnchor,
    pub context_range: [usize; 2],
    pub source_range: [usize; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentPlan {
    pub schema: String,
    pub id: String,
    pub text_version: String,
    pub prepared: PreparedText,
    pub routing: RegisterRouting,
    pub units: Vec<AnalysisUnit>,
    #[serde(skip)]
    text_offsets: Vec<usize>,
    #[serde(skip)]
    source_offsets: Vec<usize>,
}

impl DocumentPlan {
    pub fn new(document_id: Option<String>, input: &str, policy: RegisterPolicy) -> Self {
        let prepared = prepare_text(input);
        let text_version = prepared.mapping.source_sha256.clone();
        let id = document_id.unwrap_or_else(|| format!("text:{text_version}"));
        let routing = routing::route_text(&prepared.text, policy);
        let chars: Vec<_> = prepared.text.chars().collect();
        let mut graphemes = vec![0];
        for grapheme in prepared.text.graphemes(true) {
            graphemes.push(graphemes.last().unwrap() + grapheme.chars().count());
        }
        let mut sentences = vec![0];
        let mut breaks = vec![0];
        let mut lines = Vec::new();
        let mut quotes = Vec::new();
        for (index, &character) in chars.iter().enumerate() {
            match character {
                '「' => quotes.push('」'),
                '『' => quotes.push('』'),
                '」' | '』' if quotes.last() == Some(&character) => { quotes.pop(); },
                _ => {},
            }
            if matches!(character, '。' | '！' | '？' | '!' | '?' | '\n') {
                let mut end = index + 1;
                while end < chars.len() && (matches!(chars[end], '」' | '』' | '）' | ')') || chars[end].is_whitespace()) { end += 1; }
                sentences.push(end);
                if quotes.is_empty() { breaks.push(end); }
            }
            if character == '\n' { lines.push(index + 1); }
        }
        sentences.push(chars.len());
        sentences.sort_unstable(); sentences.dedup();
        let mut units = Vec::new();
        let mut start = 0;
        while start < chars.len() {
            let limit = (start + UNIT_CHARACTERS).min(chars.len());
            let end = lines.get(lines.partition_point(|&end| end <= start)).copied().filter(|&end| end <= limit).unwrap_or_else(|| {
                if limit == chars.len() { return limit; }
                breaks.get(breaks.partition_point(|&end| end <= limit).saturating_sub(1)).copied().filter(|&end| end > start + UNIT_CHARACTERS / 2)
                    .unwrap_or_else(|| {
                        let index = graphemes.partition_point(|&end| end <= limit).saturating_sub(1);
                        if graphemes[index] > start { graphemes[index] } else { graphemes[index + 1] }
                    })
            });
            let context_start = sentences.get(sentences.partition_point(|&offset| offset < start).saturating_sub(1)).copied().filter(|&offset| start - offset <= CONTEXT_CHARACTERS).unwrap_or(start);
            let context_end = sentences.get(sentences.partition_point(|&offset| offset <= end)).copied().filter(|&offset| offset - end <= CONTEXT_CHARACTERS).unwrap_or(end);
            let surface: String = chars[start..end].iter().collect();
            let anchor = OccurrenceAnchor { document_id: id.clone(), text_version: text_version.clone(), char_range: [start, end], text_sha256: text_digest(&surface) };
            let source_range = [source_boundary(&prepared.mapping, start), source_boundary(&prepared.mapping, end)];
            units.push(AnalysisUnit { id: format!("unit:{}", &text_digest(&serde_json::to_string(&anchor).unwrap())[..24]), anchor,
                context_range: [context_start, context_end], source_range });
            start = end;
        }
        let text_offsets = prepared.text.char_indices().map(|(i, _)| i).chain(std::iter::once(prepared.text.len())).collect();
        let source_offsets = input.char_indices().map(|(i, _)| i).chain(std::iter::once(input.len())).collect();
        Self { schema: VERSION.into(), id, text_version, prepared, routing, units, text_offsets, source_offsets }
    }

    pub fn unit_input(&self, index: usize) -> (PreparedText, RegisterRouting) {
        let [start, end] = self.units[index].context_range;
        let map = &self.prepared.mapping;
        let source_start = source_boundary(map, start);
        let source_end = source_boundary(map, end);
        let source_text = map.source_text[self.source_offsets[source_start]..self.source_offsets[source_end]].to_string();
        let text = self.prepared.text[self.text_offsets[start]..self.text_offsets[end]].to_string();
        let mapping = PreparationMap { schema: map.schema.clone(), source_sha256: text_digest(&source_text), text_sha256: text_digest(&text), source_text,
            origins: map.origins[start..end].iter().map(|&offset| offset - source_start).collect(),
            removed: map.removed.iter().filter(|item| item.source_range[0] >= source_start && item.source_range[1] <= source_end).map(|item| {
                let mut item = item.clone();
                item.source_range = [item.source_range[0] - source_start, item.source_range[1] - source_start];
                item.text_offset -= start;
                item
            }).collect() };
        let annotations = self.prepared.annotations.iter().filter(|a| a.char_range[0] >= start && a.char_range[1] <= end).map(|a| {
            let mut a = a.clone(); a.char_range = [a.char_range[0] - start, a.char_range[1] - start]; a
        }).collect();
        (PreparedText { text, mapping, annotations }, routing::slice(&self.routing, [start, end]))
    }
}

fn source_boundary(map: &PreparationMap, offset: usize) -> usize {
    if offset == 0 { 0 } else { map.origins.get(offset).copied().unwrap_or_else(|| map.source_text.chars().count()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotoclip_nlp::model::Register;

    #[test]
    fn repeated_text_keeps_distinct_anchors_and_context_mapping() {
        let input = "![](x.png)警察署《けいさつしょ》へ向かった。\n警察署《けいさつしょ》へ向かった。  \n";
        let plan = DocumentPlan::new(Some("book".into()), input, RegisterPolicy::Auto);
        assert_eq!(plan.units.len(), 2);
        assert_ne!(plan.units[0].id, plan.units[1].id);
        assert_eq!(plan.units[0].anchor.char_range[1], plan.units[1].anchor.char_range[0]);
        for index in 0..plan.units.len() {
            let (prepared, _) = plan.unit_input(index);
            let source: Vec<_> = prepared.mapping.source_text.chars().collect();
            assert_eq!(prepared.mapping.origins.iter().map(|&i| source[i]).collect::<String>(), prepared.text);
            assert_eq!(prepared.text, prepare_text(&prepared.mapping.source_text).text);
            assert!(!prepared.annotations.is_empty());
        }
        assert!(plan.units[1].context_range[0] < plan.units[1].anchor.char_range[0]);
        assert_eq!(plan.unit_input(1).0.text.matches("警察署").count(), 2);
    }

    #[test]
    fn long_input_is_bounded_and_quote_routing_survives_window_boundaries() {
        let input = format!("前文。「{}」という発言だった。", "か\u{3099}".repeat(12000));
        let plan = DocumentPlan::new(None, &input, RegisterPolicy::Auto);
        assert!(plan.units.len() > 20);
        assert_eq!(plan.units.last().unwrap().anchor.char_range[1], input.chars().count());
        assert!(plan.units.windows(2).all(|p| p[0].anchor.char_range[1] == p[1].anchor.char_range[0]));
        assert!(plan.units.iter().all(|unit| unit.anchor.char_range[1] - unit.anchor.char_range[0] <= UNIT_CHARACTERS));
        let (prepared, routing) = plan.unit_input(5);
        assert_ne!(prepared.text.chars().next(), Some('\u{3099}'));
        assert_eq!(routing.selected, Some(Register::Csj));
    }
}
