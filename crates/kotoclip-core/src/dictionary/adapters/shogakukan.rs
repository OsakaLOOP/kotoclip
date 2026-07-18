use super::{common, AdaptedOccurrence};
use crate::dictionary::html::{parse_fragment, HtmlElement, HtmlNode};
use crate::models::{
    DictionaryAdapterDiagnostics, DictionaryExample, DictionaryForm, DictionarySection,
    DictionarySectionItem, DictionarySense, DictionaryTag, DictionaryText,
};
use regex::Regex;
use std::sync::LazyLock;

static BRACKET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[［\[]([^］\]]{1,16})[］\]]").unwrap());
static ANGLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[〈<]([^〉>]{1,16})[〉>]").unwrap());
static PAREN_RUBY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([一-龯々〆ヵヶ]+)\(([ぁ-ゖァ-ヺー]+)\)").unwrap()
});
static FOREIGN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\[［]([^\]］]+)[\]］](.+)$").unwrap());
static SENSE_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:\d+|[①-⑳]|[一二三四五六七八九十]+|[㋐-㋾]|[A-Z])$").unwrap()
});

struct SenseDraft {
    level: usize,
    sense: DictionarySense,
}

pub fn adapt(
    indexed_headword: &str,
    _raw_headword: &str,
    structured_reading: Option<&str>,
    definition: &str,
) -> Vec<AdaptedOccurrence> {
    let root = parse_fragment(definition);
    let records = split_records(&root);
    if records.is_empty() {
        return vec![common::fallback(
            "shogakukan",
            indexed_headword,
            structured_reading,
            definition,
            "unknown",
        )];
    }
    let record_count = records.len();
    records
        .into_iter()
        .enumerate()
        .map(|(index, (heading, body))| {
            adapt_record(
                index,
                heading,
                body,
                (record_count == 1).then_some(structured_reading).flatten(),
                definition,
            )
        })
        .collect()
}

fn split_records(root: &HtmlElement) -> Vec<(&HtmlElement, Option<&HtmlElement>)> {
    let elements = root
        .children
        .iter()
        .filter_map(|child| match child {
            HtmlNode::Element(element) => Some(element),
            HtmlNode::Text(_) => None,
        })
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    for (index, element) in elements.iter().enumerate() {
        if element.name != "h3" {
            continue;
        }
        let body = elements[index + 1..]
            .iter()
            .copied()
            .find(|candidate| candidate.name == "section");
        records.push((*element, body));
    }
    records
}

fn adapt_record(
    index: usize,
    heading: &HtmlElement,
    body: Option<&HtmlElement>,
    structured_reading: Option<&str>,
    fallback_source: &str,
) -> AdaptedOccurrence {
    let display_form = common::normalize_visible_text(
        &heading.text_excluding_classes(&["pinyin_h"]),
    );
    let heading_annotation = heading
        .first_by_class("pinyin_h")
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty());
    let foreign_origin = heading_annotation
        .as_deref()
        .and_then(shogakukan_foreign_origin);
    let reading = if foreign_origin.is_some() {
        is_kana_word(&display_form).then(|| common::normalize_reading(&display_form))
    } else {
        heading_annotation
            .map(|value| common::normalize_reading(&value))
            .or_else(|| structured_reading.map(common::normalize_reading))
    };
    let mut occurrence = AdaptedOccurrence {
        source_record_index: index,
        occurrence_suffix: format!("record-{index}"),
        entry_kind: bound_kind(reading.as_deref()).to_string(),
        header: crate::models::DictionaryOccurrenceHeader {
            display_form: display_form.clone(),
            canonical_form: Some(display_form.clone()),
            reading: reading.clone(),
            origin: foreign_origin,
            scoped_forms: vec![DictionaryForm {
                form: display_form,
                reading,
                kind: "canonical".to_string(),
            }],
            ..Default::default()
        },
        diagnostics: DictionaryAdapterDiagnostics {
            coverage: "structured".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let Some(body) = body else {
        occurrence.diagnostics.coverage = "partial".to_string();
        occurrence
            .diagnostics
            .warnings
            .push("词头后没有找到对应 section".to_string());
        return common::finish(occurrence, "shogakukan", fallback_source);
    };

    let mut elements = Vec::new();
    body.all_elements(&mut elements);
    let mut main_paragraphs = Vec::new();
    collect_main_paragraphs(body, &mut main_paragraphs);
    let (senses, loose_notes) = parse_sense_paragraphs(&main_paragraphs);
    occurrence.senses = senses;

    let mut subheadwords = Vec::new();
    for element in elements.iter().copied() {
        if element.attr("data-orgtag") != Some("subhead") {
            continue;
        }
        let Some(headword) = first_by_orgtag(element, "subheadword") else {
            continue;
        };
        let (label, label_html) = subhead_label(&headword.text());
        if label.is_empty() {
            continue;
        }
        let mut content = Vec::new();
        let mut item_tags = element
            .attr("type")
            .or_else(|| headword.attr("type"))
            .map(|value| vec![common::tag("section", value)])
            .unwrap_or_default();
        let mut paragraphs = Vec::new();
        element.all_by_name("p", &mut paragraphs);
        for paragraph in paragraphs.iter().copied() {
            if paragraph.has_class("subhw_meaning") {
                let (value, tags) = subhead_content(paragraph);
                if let Some(value) = value {
                    content.push(value);
                }
                for tag in tags {
                    if !item_tags
                        .iter()
                        .any(|existing| existing.kind == tag.kind && existing.label == tag.label)
                    {
                        item_tags.push(tag);
                    }
                }
            }
        }
        let structured_paragraphs = paragraphs
            .iter()
            .copied()
            .filter(|paragraph| !paragraph.has_class("subhw_meaning"))
            .collect::<Vec<_>>();
        let (senses, _) = parse_sense_paragraphs(&structured_paragraphs);
        let examples = senses
            .is_empty()
            .then(|| standalone_examples(&structured_paragraphs))
            .unwrap_or_default();
        let nested_relation_targets = senses
            .iter()
            .flat_map(sense_relation_targets)
            .collect::<Vec<_>>();
        let relations = common::extract_links(element, "reference")
            .into_iter()
            .filter(|link| {
                !nested_relation_targets
                    .iter()
                    .any(|target| target.as_str() == link.target)
            })
            .collect();
        subheadwords.push(DictionarySectionItem {
            label: Some(label),
            label_html,
            content,
            tags: item_tags,
            examples,
            senses,
            relations,
            ..Default::default()
        });
    }
    if !subheadwords.is_empty() {
        occurrence.sections.push(DictionarySection {
            kind: "subentries".to_string(),
            label: Some("惯用与复合表达".to_string()),
            items: subheadwords,
        });
    }
    if !loose_notes.is_empty() {
        occurrence.sections.push(DictionarySection {
            kind: "notes".to_string(),
            label: Some("说明".to_string()),
            items: vec![DictionarySectionItem {
                content: loose_notes,
                ..Default::default()
            }],
        });
    }
    occurrence.links = common::extract_links(body, "reference");
    if occurrence.entry_kind == "lexical" && body.text().contains("姓氏") {
        occurrence.entry_kind = "surname".to_string();
    }
    if occurrence.senses.is_empty() && !occurrence.links.is_empty() {
        occurrence.entry_kind = "navigation".to_string();
        occurrence.diagnostics.coverage = "navigation".to_string();
    }
    common::finish(occurrence, "shogakukan", fallback_source)
}

fn bound_kind(reading: Option<&str>) -> &'static str {
    match reading {
        Some(value) if value.starts_with('-') && value.ends_with('-') => "bound_morpheme",
        Some(value) if value.starts_with('-') => "suffix",
        Some(value) if value.ends_with('-') => "prefix",
        _ => "lexical",
    }
}

fn shogakukan_foreign_origin(value: &str) -> Option<String> {
    let captures = FOREIGN_RE.captures(value)?;
    let code = captures.get(1)?.as_str().trim();
    let word = captures.get(2)?.as_str().trim();
    if word.is_empty() || !word.chars().any(|character| character.is_ascii_alphabetic()) {
        return None;
    }
    let language = match code {
        "フ" | "仏" => "法语",
        "英" => "英语",
        "独" => "德语",
        "伊" => "意大利语",
        _ => code,
    };
    Some(format!("{language} {word}"))
}

fn is_kana_word(value: &str) -> bool {
    value.chars().all(|character| {
        character.is_whitespace()
            || matches!(character, '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}')
    })
}

fn first_marker(element: &HtmlElement) -> Option<String> {
    ["black-square", "white-square"]
        .into_iter()
        .find_map(|class| element.first_by_class(class))
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| SENSE_MARKER_RE.is_match(value))
}

fn first_typed_text(element: &HtmlElement, target_type: &str) -> Option<String> {
    let mut elements = Vec::new();
    element.all_elements(&mut elements);
    elements
        .into_iter()
        .find(|candidate| candidate.attr("type") == Some(target_type))
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())
}

fn first_named_text(element: &HtmlElement, name: &str) -> Option<String> {
    let mut elements = Vec::new();
    element.all_by_name(name, &mut elements);
    elements
        .first()
        .map(|element| element.text())
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())
}

fn parse_sense_paragraphs(
    paragraphs: &[&HtmlElement],
) -> (Vec<DictionarySense>, Vec<DictionaryText>) {
    let mut drafts: Vec<SenseDraft> = Vec::new();
    let mut loose_notes = Vec::new();
    for element in paragraphs {
        match element.attr("data-orgtag") {
            Some("meaning") => {
                let meaning_type = element.attr("type").unwrap_or_default();
                if matches!(meaning_type, "補足" | "注意") {
                    let note = common::text("ja", element.text());
                    if let Some(last) = drafts.last_mut() {
                        last.sense.notes.push(note);
                    } else {
                        loose_notes.push(note);
                    }
                    continue;
                }
                let level = element
                    .attr("level")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(1);
                let marker = element
                    .attr("no")
                    .map(str::to_string)
                    .or_else(|| first_marker(element));
                let heading_text = first_typed_text(element, "語義区分2");
                let (glosses, mut residual, tags) =
                    meaning_payload(element, marker.as_deref());
                if let Some(heading) = &heading_text {
                    residual = remove_once(&residual, heading);
                }
                let relations = common::extract_links(element, "reference");
                for relation in &relations {
                    residual = remove_once(&residual, &relation.label);
                }
                if !relations.is_empty() {
                    residual = residual
                        .replace('⇒', "")
                        .replace('→', "")
                        .replace('☞', "");
                    residual = trim_outer_punctuation(&residual);
                }
                let mut sense = DictionarySense {
                    sense_id: format!("s{}", drafts.len() + 1),
                    marker,
                    heading: heading_text,
                    glosses,
                    tags,
                    relations,
                    ..Default::default()
                };
                if !residual.is_empty() {
                    if sense.glosses.is_empty() && !contains_kana(&residual) {
                        sense.glosses.extend(
                            split_top_level_phrases(&residual)
                                .into_iter()
                                .map(|value| common::text("zh-CN", value)),
                        );
                    } else {
                        sense.definitions.push(common::text("ja", residual));
                    }
                }
                drafts.push(SenseDraft { level, sense });
            }
            Some("example") => {
                let source = first_named_text(element, "jae");
                let translation = first_named_text(element, "ja_cn");
                if let (Some(last), Some(example)) = (
                    drafts.last_mut(),
                    source
                        .as_deref()
                        .and_then(|source| common::example(source, translation.as_deref())),
                ) {
                    last.sense.examples.push(example);
                }
            }
            _ => {}
        }
    }
    (build_sense_tree(drafts), loose_notes)
}

fn standalone_examples(paragraphs: &[&HtmlElement]) -> Vec<DictionaryExample> {
    paragraphs
        .iter()
        .filter(|paragraph| paragraph.attr("data-orgtag") == Some("example"))
        .filter_map(|paragraph| {
            let source = first_named_text(paragraph, "jae")?;
            let translation = first_named_text(paragraph, "ja_cn");
            common::example(&source, translation.as_deref())
        })
        .collect()
}

fn sense_relation_targets(sense: &DictionarySense) -> Box<dyn Iterator<Item = &String> + '_> {
    Box::new(
        sense
            .relations
            .iter()
            .map(|link| &link.target)
            .chain(sense.children.iter().flat_map(sense_relation_targets)),
    )
}

fn contains_kana(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'
        )
    })
}

fn subhead_content(
    paragraph: &HtmlElement,
) -> (Option<DictionaryText>, Vec<DictionaryTag>) {
    let mut value = common::normalize_visible_text(&paragraph.text());
    let mut tags = Vec::new();
    for class in ["white-square", "black-square"] {
        let mut elements = Vec::new();
        paragraph.all_by_class(class, &mut elements);
        for element in elements {
            let label = common::normalize_visible_text(&element.text());
            if label.is_empty() {
                continue;
            }
            value = remove_once(&value, &label);
            push_unique_tag(&mut tags, "register", &label);
        }
    }
    let domain_labels = ANGLE_RE
        .captures_iter(&value)
        .filter_map(|captures| captures.get(1))
        .map(|label| common::normalize_visible_text(label.as_str()))
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    value = ANGLE_RE.replace_all(&value, "").to_string();
    for label in domain_labels {
        push_unique_tag(&mut tags, "domain", &label);
    }
    let value = trim_outer_punctuation(&value);
    let content = (!value.is_empty()).then(|| common::text("zh-CN", value));
    (content, tags)
}

fn subhead_label(value: &str) -> (String, Option<String>) {
    let value = common::normalize_visible_text(value);
    let mut html = String::new();
    let mut plain = String::new();
    let mut last = 0usize;
    let mut found = false;
    for captures in PAREN_RUBY_RE.captures_iter(&value) {
        let Some(whole) = captures.get(0) else {
            continue;
        };
        let base = captures.get(1).map(|value| value.as_str()).unwrap_or_default();
        let reading = captures.get(2).map(|value| value.as_str()).unwrap_or_default();
        html.push_str(&common::escape_html(&value[last..whole.start()]));
        plain.push_str(&value[last..whole.start()]);
        html.push_str("<ruby>");
        html.push_str(&common::escape_html(base));
        html.push_str("<rt>");
        html.push_str(&common::escape_html(reading));
        html.push_str("</rt></ruby>");
        plain.push_str(base);
        last = whole.end();
        found = true;
    }
    html.push_str(&common::escape_html(&value[last..]));
    plain.push_str(&value[last..]);
    let label_html = found.then(|| common::sanitize_fallback(&html));
    (plain, label_html)
}

fn collect_main_paragraphs<'a>(element: &'a HtmlElement, output: &mut Vec<&'a HtmlElement>) {
    for child in common::direct_child_elements(element) {
        if matches!(
            child.attr("data-orgtag"),
            Some("subhead" | "subheadword")
        ) {
            continue;
        }
        if child.name == "p" {
            output.push(child);
        } else {
            collect_main_paragraphs(child, output);
        }
    }
}

fn first_by_orgtag<'a>(element: &'a HtmlElement, tag: &str) -> Option<&'a HtmlElement> {
    if element.attr("data-orgtag") == Some(tag) {
        return Some(element);
    }
    common::direct_child_elements(element).find_map(|child| first_by_orgtag(child, tag))
}

fn meaning_payload(
    element: &HtmlElement,
    marker: Option<&str>,
) -> (Vec<DictionaryText>, String, Vec<DictionaryTag>) {
    let visible = common::normalize_visible_text(&element.text());
    let mut working_visible = visible.clone();
    if let Some(heading) = first_typed_text(element, "語義区分2") {
        working_visible = remove_once(&working_visible, &heading);
    }
    let mut tags = Vec::new();
    let bracket_labels = BRACKET_RE
        .captures_iter(&working_visible)
        .filter_map(|captures| captures.get(1))
        .map(|value| common::normalize_visible_text(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    for label in bracket_labels.iter().filter(|label| is_pos_label(label)) {
        push_unique_tag(&mut tags, "pos", label);
    }
    for captures in ANGLE_RE.captures_iter(&working_visible) {
        if let Some(value) = captures.get(1) {
            let label = common::normalize_visible_text(value.as_str());
            if !label.is_empty() {
                push_unique_tag(&mut tags, "domain", &label);
            }
        }
    }
    let mut inline_labels = Vec::new();
    for class in ["white-square", "black-square"] {
        let mut elements = Vec::new();
        element.all_by_class(class, &mut elements);
        for label_element in elements {
            let label = common::normalize_visible_text(&label_element.text());
            if label.is_empty() || marker == Some(label.as_str()) {
                continue;
            }
            if !inline_labels.contains(&label) {
                inline_labels.push(label.clone());
            }
            push_unique_tag(&mut tags, "register", &label);
        }
    }

    let bold_values = bold_values(element, marker);
    let semantic_qualifiers = bracket_labels
        .iter()
        .any(|label| !is_pos_label(label));
    let mut residual = strip_meaning_scaffolding(&working_visible, marker, &bold_values);
    for label in &inline_labels {
        residual = remove_once(&residual, label);
    }
    if !has_lexical_content(&residual) {
        residual.clear();
    }
    let glosses = if semantic_qualifiers {
        residual.clear();
        qualified_visible_glosses(&working_visible, marker, &inline_labels)
    } else {
        let mut without_bold = working_visible.clone();
        if let Some(marker) = marker {
            without_bold = remove_once(&without_bold, marker);
        }
        without_bold = ANGLE_RE.replace_all(&without_bold, "").to_string();
        without_bold = BRACKET_RE.replace_all(&without_bold, "").to_string();
        for label in &inline_labels {
            without_bold = remove_once(&without_bold, label);
        }
        for value in &bold_values {
            without_bold = remove_once(&without_bold, value);
        }
        let has_parenthetical_group = working_visible.contains('（')
            || working_visible.contains('(');
        if !bold_values.is_empty()
            && has_parenthetical_group
            && !has_lexical_content(&without_bold)
        {
            let mut phrase = working_visible.clone();
            if let Some(marker) = marker {
                phrase = remove_once(&phrase, marker);
            }
            phrase = ANGLE_RE.replace_all(&phrase, "").to_string();
            phrase = BRACKET_RE.replace_all(&phrase, "").to_string();
            for label in &inline_labels {
                phrase = remove_once(&phrase, label);
            }
            residual.clear();
            phrase
                .split(['；', ';'])
                .map(trim_outer_punctuation)
                .filter(|value| !value.is_empty())
                .map(|value| common::text("zh-CN", value))
                .collect()
        } else {
            bold_values
                .iter()
                .map(|value| common::text("zh-CN", value))
                .collect()
        }
    };
    (glosses, residual, tags)
}

fn bold_values(element: &HtmlElement, marker: Option<&str>) -> Vec<String> {
    let mut bold = Vec::new();
    element.all_by_name("b", &mut bold);
    let mut values = Vec::new();
    for item in bold {
        let value = common::normalize_visible_text(&item.text());
        if value.is_empty() || marker == Some(value.as_str()) || values.contains(&value) {
            continue;
        }
        values.push(value);
    }
    values
}

fn qualified_visible_glosses(
    visible: &str,
    marker: Option<&str>,
    inline_labels: &[String],
) -> Vec<DictionaryText> {
    let mut source = visible.to_string();
    if let Some(marker) = marker {
        source = remove_once(&source, marker);
    }
    source = ANGLE_RE.replace_all(&source, "").to_string();
    for label in inline_labels {
        source = remove_once(&source, label);
    }
    let pos_brackets = BRACKET_RE
        .captures_iter(&source)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let label = captures.get(1)?;
            is_pos_label(&common::normalize_visible_text(label.as_str()))
                .then_some(whole.as_str().to_string())
        })
        .collect::<Vec<_>>();
    for bracket in pos_brackets {
        source = remove_once(&source, &bracket);
    }
    let semantic = BRACKET_RE
        .captures_iter(&source)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let label = captures.get(1)?;
            let label = common::normalize_visible_text(label.as_str());
            (!label.is_empty() && !is_pos_label(&label))
                .then_some((whole.start(), whole.end(), label))
        })
        .collect::<Vec<_>>();
    let mut glosses = Vec::new();
    if let Some((first_start, _, _)) = semantic.first() {
        push_visible_phrases(&source[..*first_start], None, &mut glosses);
    }
    for (index, (_, end, qualifier)) in semantic.iter().enumerate() {
        let next_start = semantic
            .get(index + 1)
            .map(|(start, _, _)| *start)
            .unwrap_or(source.len());
        push_visible_phrases(&source[*end..next_start], Some(qualifier), &mut glosses);
    }
    glosses
}

fn push_visible_phrases(
    source: &str,
    qualifier: Option<&String>,
    output: &mut Vec<DictionaryText>,
) {
    for phrase in split_top_level_phrases(source) {
        let item = qualifier
            .map(|label| common::qualified_text("zh-CN", label, &phrase))
            .unwrap_or_else(|| common::text("zh-CN", &phrase));
        if !output.iter().any(|existing| {
            existing.qualifier == item.qualifier && existing.html == item.html
        }) {
            output.push(item);
        }
    }
}

fn split_top_level_phrases(source: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for character in source.chars() {
        match character {
            '（' | '(' => {
                depth += 1;
                current.push(character);
            }
            '）' | ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            '，' | ',' | '；' | ';' if depth == 0 => {
                let phrase = trim_outer_punctuation(&current);
                if !phrase.is_empty() {
                    phrases.push(phrase);
                }
                current.clear();
            }
            _ => current.push(character),
        }
    }
    let phrase = trim_outer_punctuation(&current);
    if !phrase.is_empty() {
        phrases.push(phrase);
    }
    phrases
}

fn strip_meaning_scaffolding(
    visible: &str,
    marker: Option<&str>,
    bold_values: &[String],
) -> String {
    let mut residual = visible.to_string();
    if let Some(marker) = marker {
        residual = remove_once(&residual, marker);
    }
    residual = ANGLE_RE.replace_all(&residual, "").to_string();
    residual = BRACKET_RE.replace_all(&residual, "").to_string();
    for value in bold_values {
        residual = remove_once(&residual, value);
    }
    let residual = trim_outer_punctuation(&residual);
    has_lexical_content(&residual)
        .then_some(residual)
        .unwrap_or_default()
}

fn trim_outer_punctuation(value: &str) -> String {
    common::normalize_visible_text(value)
        .trim_matches(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '，' | ',' | '；' | ';' | '。' | '．' | '、'
                )
        })
        .to_string()
}

fn has_lexical_content(value: &str) -> bool {
    value.chars().any(|character| character.is_alphanumeric())
}

fn is_pos_label(label: &str) -> bool {
    matches!(
        label,
        "名" | "名詞" | "代" | "代名詞" | "動" | "自動" | "他動" | "形" | "形動"
            | "副" | "連体" | "連体詞" | "接続" | "接続詞" | "感" | "感動詞" | "助"
            | "助詞" | "助動" | "助動詞" | "接頭" | "接頭語" | "接尾" | "接尾語"
    )
}

fn push_unique_tag(tags: &mut Vec<DictionaryTag>, kind: &str, label: &str) {
    if !tags.iter().any(|tag| tag.kind == kind && tag.label == label) {
        tags.push(common::tag(kind, label));
    }
}

fn remove_once(source: &str, value: &str) -> String {
    source.replacen(value, "", 1)
}

fn build_sense_tree(drafts: Vec<SenseDraft>) -> Vec<DictionarySense> {
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, DictionarySense)> = Vec::new();
    for draft in drafts {
        while stack.last().is_some_and(|(level, _)| *level >= draft.level) {
            pop_sense(&mut stack, &mut roots);
        }
        stack.push((draft.level, draft.sense));
    }
    while !stack.is_empty() {
        pop_sense(&mut stack, &mut roots);
    }
    roots
}

fn pop_sense(stack: &mut Vec<(usize, DictionarySense)>, roots: &mut Vec<DictionarySense>) {
    let (_, sense) = stack.pop().expect("stack is not empty");
    if let Some((_, parent)) = stack.last_mut() {
        parent.children.push(sense);
    } else {
        roots.push(sense);
    }
}
