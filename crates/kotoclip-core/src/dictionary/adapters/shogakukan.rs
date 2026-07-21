use super::{common, AdaptedOccurrence};
use crate::dictionary::html::{parse_fragment, HtmlElement, HtmlNode};
use crate::models::{
    DictionaryAdapterDiagnostics, DictionaryExample, DictionaryForm, DictionaryGlossClause,
    DictionaryGlossGroup, DictionarySection, DictionarySectionItem, DictionarySense, DictionaryTag,
    DictionaryText,
};
use crate::text_language::is_japanese_text;
use regex::Regex;
use std::sync::LazyLock;

static ANGLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[〈<]([^〉>]{1,16})[〉>]").unwrap());
static PAREN_RUBY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([一-龯々〆ヵヶ]+)\(([ぁ-ゖァ-ヺー]+)\)").unwrap());
static FOREIGN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\[［]([^\]］]+)[\]］](.+)$").unwrap());
static SENSE_MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\d+|[①-⑳]|[一二三四五六七八九十]+|[㋐-㋾]|[A-Z])$").unwrap());

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
        let standalone_subheads = common::direct_child_elements(&root)
            .filter(|element| element.attr("data-orgtag") == Some("subhead"))
            .collect::<Vec<_>>();
        if !standalone_subheads.is_empty() {
            return standalone_subheads
                .into_iter()
                .enumerate()
                .map(|(index, subhead)| {
                    adapt_standalone_subhead(
                        index,
                        indexed_headword,
                        subhead,
                        &root,
                        structured_reading,
                        definition,
                    )
                })
                .collect();
        }
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

fn adapt_standalone_subhead(
    index: usize,
    indexed_headword: &str,
    subhead: &HtmlElement,
    root: &HtmlElement,
    structured_reading: Option<&str>,
    fallback_source: &str,
) -> AdaptedOccurrence {
    let (display_form, _) = first_by_orgtag(subhead, "subheadword")
        .map(HtmlElement::text)
        .map(|value| subhead_label(&value))
        .unwrap_or_else(|| (indexed_headword.to_string(), None));
    let display_form = if display_form.is_empty() {
        indexed_headword.to_string()
    } else {
        display_form
    };
    let entry_kind = match subhead.attr("type") {
        Some("慣用句") => "phrase",
        _ => "lexical",
    };
    let reading = structured_reading.map(common::normalize_reading);
    let mut scoped_forms = vec![DictionaryForm {
        form: display_form.clone(),
        reading: reading.clone(),
        kind: "canonical".to_string(),
    }];
    for original in common::direct_child_elements(subhead)
        .filter(|element| {
            element.attr("data-orgtag") == Some("subheadword")
                && element.attr("type") == Some("複合語原綴")
        })
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty() && value != &display_form)
    {
        if !scoped_forms.iter().any(|form| form.form == original) {
            scoped_forms.push(DictionaryForm {
                form: original,
                reading: None,
                kind: "original".to_string(),
            });
        }
    }
    let mut occurrence = AdaptedOccurrence {
        source_record_index: index,
        occurrence_suffix: format!("standalone-subhead-{index}"),
        entry_kind: entry_kind.to_string(),
        header: crate::models::DictionaryOccurrenceHeader {
            display_form: display_form.clone(),
            canonical_form: Some(display_form.clone()),
            reading: reading.clone(),
            scoped_forms,
            ..Default::default()
        },
        diagnostics: DictionaryAdapterDiagnostics {
            coverage: "structured".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut paragraphs = Vec::new();
    collect_main_paragraphs(subhead, &mut paragraphs);
    let (senses, loose_notes) = parse_sense_paragraphs(&paragraphs);
    occurrence.senses = senses;
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
    occurrence.links = common::extract_links(root, "reference");
    common::finish(occurrence, "shogakukan", fallback_source)
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
    let display_form =
        common::normalize_visible_text(&heading.text_excluding_classes(&["pinyin_h"]));
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
    if word.is_empty()
        || !word
            .chars()
            .any(|character| character.is_ascii_alphabetic())
    {
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
                let (gloss_groups, mut residual, tags) =
                    meaning_payload(element, marker.as_deref());
                let glosses = legacy_glosses(&gloss_groups);
                if let Some(heading) = &heading_text {
                    residual = remove_once(&residual, heading);
                }
                let relations = common::extract_links(element, "reference");
                for relation in &relations {
                    residual = remove_once(&residual, &relation.label);
                }
                if !relations.is_empty() {
                    residual = residual.replace('⇒', "").replace('→', "").replace('☞', "");
                    residual = trim_outer_punctuation(&residual);
                }
                let mut sense = DictionarySense {
                    sense_id: format!("s{}", drafts.len() + 1),
                    marker,
                    heading: heading_text,
                    glosses,
                    gloss_groups,
                    tags,
                    relations,
                    ..Default::default()
                };
                if !residual.is_empty() {
                    if sense.glosses.is_empty() && !is_japanese_text(&residual) {
                        sense.glosses.extend(
                            split_top_level_phrases(&residual)
                                .into_iter()
                                .map(|value| common::text("zh-CN", value)),
                        );
                    } else {
                        sense.definitions.push(common::text("ja", residual));
                    }
                }
                if let Some(previous) = drafts.last_mut().filter(|previous| {
                    sense.marker.is_some()
                        && previous.level == level
                        && previous.sense.marker == sense.marker
                }) {
                    merge_repeated_sense_group(&mut previous.sense, sense);
                } else {
                    drafts.push(SenseDraft { level, sense });
                }
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

fn subhead_content(paragraph: &HtmlElement) -> (Option<DictionaryText>, Vec<DictionaryTag>) {
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
        let base = captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let reading = captures
            .get(2)
            .map(|value| value.as_str())
            .unwrap_or_default();
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
        if matches!(child.attr("data-orgtag"), Some("subhead" | "subheadword")) {
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
) -> (Vec<DictionaryGlossGroup>, String, Vec<DictionaryTag>) {
    let mut builder = GlossGroupBuilder::default();
    collect_gloss_nodes(&element.children, marker, &mut builder);
    let groups = builder.finish();
    let mut tags = Vec::new();
    for group in &groups {
        for clause in &group.clauses {
            for tag in clause.leading_tags.iter().chain(&clause.trailing_tags) {
                push_unique_tag(&mut tags, &tag.kind, &tag.label);
            }
        }
    }
    let residual = if groups.iter().any(|group| {
        group
            .clauses
            .iter()
            .any(|clause| !clause.text.html.is_empty())
    }) {
        String::new()
    } else {
        let mut value = common::normalize_visible_text(&element.text());
        if let Some(marker) = marker {
            value = remove_once(&value, marker);
        }
        for group in &groups {
            if let Some(heading) = &group.heading {
                value = remove_once(&value, heading);
            }
        }
        trim_outer_punctuation(&value)
    };
    (groups, residual, tags)
}

#[derive(Default)]
struct GlossGroupBuilder {
    groups: Vec<DictionaryGlossGroup>,
    current_group: DictionaryGlossGroup,
    current_clause: DictionaryGlossClause,
    pending_separator: Option<String>,
}

impl GlossGroupBuilder {
    fn heading(&mut self, value: &str) {
        self.flush_group();
        let value = common::normalize_visible_text(value);
        self.current_group.heading = (!value.is_empty()).then_some(value);
    }

    fn qualifier(&mut self, value: &str) {
        self.flush_clause();
        let value = common::normalize_visible_text(value);
        self.current_clause.qualifier = (!value.is_empty()).then_some(value);
    }

    fn tag(&mut self, kind: &str, label: &str) {
        let label = common::normalize_visible_text(label);
        if label.is_empty() {
            return;
        }
        let tag = common::tag(kind, label);
        let target = if self.current_clause.text.html.trim().is_empty() {
            &mut self.current_clause.leading_tags
        } else {
            &mut self.current_clause.trailing_tags
        };
        if !target
            .iter()
            .any(|existing| existing.kind == tag.kind && existing.label == tag.label)
        {
            target.push(tag);
        }
    }

    fn text(&mut self, value: &str) {
        if !self.current_clause.trailing_tags.is_empty()
            && value.chars().any(|character| !character.is_whitespace())
        {
            self.flush_clause();
        }
        self.current_clause.text.html.push_str(value);
    }

    fn separator(&mut self, value: char) {
        self.flush_clause();
        self.pending_separator = Some(value.to_string());
    }

    fn flush_clause(&mut self) {
        let raw = normalize_gloss_text(&self.current_clause.text.html);
        let has_tags = !self.current_clause.leading_tags.is_empty()
            || !self.current_clause.trailing_tags.is_empty();
        if raw.is_empty() && self.current_clause.qualifier.is_none() && !has_tags {
            self.current_clause = DictionaryGlossClause::default();
            return;
        }
        self.current_clause.separator = self.pending_separator.take();
        self.current_clause.text = if raw.is_empty() {
            DictionaryText::default()
        } else {
            common::text(
                if is_japanese_text(&raw) {
                    "ja"
                } else {
                    "zh-CN"
                },
                raw,
            )
        };
        self.current_group
            .clauses
            .push(std::mem::take(&mut self.current_clause));
    }

    fn flush_group(&mut self) {
        self.flush_clause();
        if self.current_group.heading.is_some() || !self.current_group.clauses.is_empty() {
            self.groups.push(std::mem::take(&mut self.current_group));
        }
        self.pending_separator = None;
    }

    fn finish(mut self) -> Vec<DictionaryGlossGroup> {
        self.flush_group();
        self.groups
            .into_iter()
            .filter(|group| {
                group.clauses.iter().any(|clause| {
                    !clause.text.html.is_empty()
                        || !clause.leading_tags.is_empty()
                        || !clause.trailing_tags.is_empty()
                })
            })
            .collect()
    }
}

fn collect_gloss_nodes(nodes: &[HtmlNode], marker: Option<&str>, builder: &mut GlossGroupBuilder) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => collect_gloss_text(text, builder),
            HtmlNode::Element(element) if element.attr("type") == Some("語義区分2") => {
                builder.heading(&element.text());
            }
            HtmlNode::Element(element)
                if element.has_class("white-square") || element.has_class("black-square") =>
            {
                let label = common::normalize_visible_text(&element.text());
                if !label.is_empty() && marker != Some(label.as_str()) {
                    builder.tag(inline_tag_kind(&label), &label);
                }
            }
            HtmlNode::Element(element) if element.name == "a" => {}
            HtmlNode::Element(element) if element.name == "b" => {
                let value = common::normalize_visible_text(&element.text());
                if marker != Some(value.as_str()) {
                    collect_gloss_nodes(&element.children, marker, builder);
                }
            }
            HtmlNode::Element(element) if element.name == "br" => builder.text(" "),
            HtmlNode::Element(element) => collect_gloss_nodes(&element.children, marker, builder),
        }
    }
}

fn collect_gloss_text(source: &str, builder: &mut GlossGroupBuilder) {
    let mut buffer = String::new();
    let mut depth = 0usize;
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let character = chars[index];
        let bracket = match character {
            '［' => Some(('］', "square")),
            '[' => Some((']', "square")),
            '〈' => Some(('〉', "angle")),
            '〔' => Some(('〕', "round")),
            _ => None,
        };
        if depth == 0 {
            if let Some((closing, kind)) = bracket {
                if let Some(relative_end) = chars[index + 1..]
                    .iter()
                    .position(|candidate| *candidate == closing)
                {
                    flush_gloss_buffer(&mut buffer, builder);
                    let end = index + 1 + relative_end;
                    let label = chars[index + 1..end].iter().collect::<String>();
                    match kind {
                        "square" if is_pos_label(&common::normalize_visible_text(&label)) => {
                            builder.tag("pos", &label)
                        }
                        "square" => builder.qualifier(&label),
                        "angle" => builder.tag("domain", &label),
                        "round" if is_japanese_text(&label) => builder.heading(&label),
                        _ => {
                            buffer.push(character);
                            buffer.push_str(&label);
                            buffer.push(closing);
                        }
                    }
                    index = end + 1;
                    continue;
                }
            }
            if matches!(character, '；' | ';') {
                flush_gloss_buffer(&mut buffer, builder);
                builder.separator('；');
                index += 1;
                continue;
            }
        }
        match character {
            '（' | '(' => depth += 1,
            '）' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        buffer.push(character);
        index += 1;
    }
    flush_gloss_buffer(&mut buffer, builder);
}

fn flush_gloss_buffer(buffer: &mut String, builder: &mut GlossGroupBuilder) {
    if !buffer.is_empty() {
        builder.text(buffer);
        buffer.clear();
    }
}

fn normalize_gloss_text(value: &str) -> String {
    common::normalize_visible_text(value)
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, '。' | '．' | '⇒' | '→' | '☞')
        })
        .to_string()
}

fn inline_tag_kind(label: &str) -> &'static str {
    if is_pos_label(label) {
        "pos"
    } else if matches!(label, "成語" | "書面語" | "口語" | "俗語" | "方言") {
        "register"
    } else {
        "usage"
    }
}

fn legacy_glosses(groups: &[DictionaryGlossGroup]) -> Vec<DictionaryText> {
    groups
        .iter()
        .flat_map(|group| &group.clauses)
        .filter(|clause| !clause.text.html.is_empty())
        .map(|clause| {
            let mut text = clause.text.clone();
            text.qualifier = clause.qualifier.clone();
            text
        })
        .collect()
}

fn merge_repeated_sense_group(target: &mut DictionarySense, mut source: DictionarySense) {
    target.gloss_groups.append(&mut source.gloss_groups);
    target.glosses.append(&mut source.glosses);
    for tag in source.tags {
        if !target
            .tags
            .iter()
            .any(|existing| existing.kind == tag.kind && existing.label == tag.label)
        {
            target.tags.push(tag);
        }
    }
    target.definitions.append(&mut source.definitions);
    target.examples.append(&mut source.examples);
    target.notes.append(&mut source.notes);
    target.relations.append(&mut source.relations);
    target.children.append(&mut source.children);
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

fn trim_outer_punctuation(value: &str) -> String {
    common::normalize_visible_text(value)
        .trim_matches(|character: char| {
            character.is_whitespace()
                || matches!(character, '，' | ',' | '；' | ';' | '。' | '．' | '、')
        })
        .to_string()
}

fn is_pos_label(label: &str) -> bool {
    matches!(
        label,
        "名" | "名詞"
            | "代"
            | "代名詞"
            | "動"
            | "自動"
            | "他動"
            | "形"
            | "形動"
            | "副"
            | "連体"
            | "連体詞"
            | "接続"
            | "接続詞"
            | "感"
            | "感動詞"
            | "助"
            | "助詞"
            | "助動"
            | "助動詞"
            | "接頭"
            | "接頭語"
            | "接尾"
            | "接尾語"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapt_one(definition: &str) -> AdaptedOccurrence {
        adapt("测试", "测试", None, definition)
            .into_iter()
            .next()
            .expect("应生成 occurrence")
    }

    #[test]
    fn keeps_parenthetical_translation_and_commas_in_one_clause() {
        let occurrence = adapt_one(
            r#"<h3>振り返る<span class="pinyin_h">ふりかえる</span></h3><section class="description"><p data-orgtag="meaning" level="2" no="1"><b>1</b><span type="語義区分2">〔後ろを〕</span><b>回头看</b>，<b>回过头去</b>（<b>看</b>），<b>向后</b>（<b>看</b>）．</p></section>"#,
        );
        let group = &occurrence.senses[0].gloss_groups[0];
        assert_eq!(group.heading.as_deref(), Some("〔後ろを〕"));
        assert_eq!(group.clauses.len(), 1);
        assert_eq!(
            group.clauses[0].text.html,
            "回头看，回过头去（看），向后（看）"
        );
        assert!(occurrence.senses[0].definitions.is_empty());
    }

    #[test]
    fn preserves_internal_qualifiers_semicolons_and_trailing_tags() {
        let occurrence = adapt_one(
            r#"<h3>鍵<span class="pinyin_h">かぎ</span></h3><section class="description"><p data-orgtag="meaning" level="2" no="1"><b>1</b><span type="語義区分2">〔ドアなどの〕</span>［キー］<b>钥匙</b>；［錠前］<b>锁</b><span class="white-square">成語</span>．</p></section>"#,
        );
        let clauses = &occurrence.senses[0].gloss_groups[0].clauses;
        assert_eq!(clauses.len(), 2);
        assert_eq!(clauses[0].qualifier.as_deref(), Some("キー"));
        assert_eq!(clauses[0].text.html, "钥匙");
        assert_eq!(clauses[1].separator.as_deref(), Some("；"));
        assert_eq!(clauses[1].qualifier.as_deref(), Some("錠前"));
        assert_eq!(clauses[1].text.html, "锁");
        assert_eq!(clauses[1].trailing_tags[0].label, "成語");
    }

    #[test]
    fn merges_repeated_marker_paragraphs_as_multiple_groups() {
        let occurrence = adapt_one(
            r#"<h3>キック<span class="pinyin_h">きっく</span></h3><section class="description"><p data-orgtag="meaning" level="2" no="1"><b>1</b><span type="語義区分2">〔球技などで〕</span><b>踢球</b>．</p><p data-orgtag="meaning" level="2" no="1"><b>1</b><span type="語義区分2">〔反動・反発〕</span>反弹；反冲．</p></section>"#,
        );
        assert_eq!(occurrence.senses.len(), 1);
        assert_eq!(occurrence.senses[0].gloss_groups.len(), 2);
        assert_eq!(
            occurrence.senses[0].gloss_groups[1].heading.as_deref(),
            Some("〔反動・反発〕")
        );
    }

    #[test]
    fn adapts_standalone_subhead_as_a_structured_occurrence() {
        let definition = r#"<link rel="stylesheet" type="text/css" href="Shogakukanjcv3.css"><div data-orgtag="subhead" id="JCD3_320300_SC080" type="複合語" delimiter="┃"><div data-orgtag="subheadword" type="複合語">世間話</div><p data-orgtag="meaning" class="subhw_meaning">闲话；［世間話をする］闲聊，闲谈，聊天儿<span class="white-square">口語</span>，拉〔扯〕家常<span class="white-square">口語</span>；张家长李家短．</p><p data-orgtag="example" delimiter="¶"><jae>30分ほど世間話で過ごした</jae><ja_cn>随便闲聊了三十来分钟．</ja_cn></p><p data-orgtag="example" delimiter="¶"><jae>友だちと世間話をさかなに一杯やった</jae><ja_cn>跟朋友以清谈佐酒喝了两杯．</ja_cn></p></div>参见：<a href="entry://世間">世間</a>"#;
        let occurrence = adapt("世間話", "世間話", None, definition)
            .into_iter()
            .next()
            .expect("应生成独立 occurrence");

        assert_eq!(occurrence.header.display_form, "世間話");
        assert_eq!(occurrence.entry_kind, "lexical");
        assert_eq!(occurrence.diagnostics.coverage, "structured");
        assert_eq!(occurrence.senses.len(), 1);
        let clauses = &occurrence.senses[0].gloss_groups[0].clauses;
        assert_eq!(clauses.len(), 4);
        assert_eq!(clauses[1].qualifier.as_deref(), Some("世間話をする"));
        assert_eq!(clauses[1].trailing_tags[0].label, "口語");
        assert_eq!(clauses[2].trailing_tags[0].label, "口語");
        assert_eq!(occurrence.senses[0].examples.len(), 2);
        assert!(occurrence.links.iter().any(|link| link.target == "世間"));
        assert!(!occurrence.definition_html.contains("subheadword"));
    }

    #[test]
    fn recognizes_kanji_only_japanese_gloss_heading() {
        let occurrence = adapt_one(
            r#"<h3>雑談<span class="pinyin_h">ざつだん</span></h3><section class="description"><p data-orgtag="meaning">〔世間話〕<b>闲聊</b>．</p></section>"#,
        );
        assert_eq!(
            occurrence.senses[0].gloss_groups[0].heading.as_deref(),
            Some("世間話")
        );
        assert_eq!(
            occurrence.senses[0].gloss_groups[0].clauses[0].text.lang,
            "zh-CN"
        );
    }

    #[test]
    fn preserves_standalone_idiom_kind_and_example() {
        let definition = r#"<div data-orgtag="subhead" type="慣用句"><div data-orgtag="subheadword" type="慣用句">…たことがある</div><p data-orgtag="meaning" class="subhw_meaning">（曾经）……过．</p><p data-orgtag="example"><jae>行ったことがある</jae><ja_cn>去过．</ja_cn></p></div>参见：<a href="entry://た">た</a>"#;
        let occurrence = adapt("…たことがある", "…たことがある", None, definition)
            .into_iter()
            .next()
            .expect("应生成惯用句 occurrence");

        assert_eq!(occurrence.entry_kind, "phrase");
        assert_eq!(occurrence.senses.len(), 1);
        assert_eq!(occurrence.senses[0].examples.len(), 1);
        assert!(occurrence.links.iter().any(|link| link.target == "た"));
    }

    #[test]
    fn preserves_standalone_original_spelling() {
        let definition = r#"<div data-orgtag="subhead" type="複合語"><div data-orgtag="subheadword" type="複合語">ペーパーワーク</div><div data-orgtag="subheadword" type="複合語原綴">paperwork</div><p data-orgtag="meaning" class="subhw_meaning">書類仕事．</p></div>"#;
        let occurrence = adapt("ペーパーワーク", "ペーパーワーク", None, definition)
            .into_iter()
            .next()
            .expect("应生成独立 occurrence");

        assert_eq!(occurrence.header.scoped_forms.len(), 2);
        assert_eq!(occurrence.header.scoped_forms[1].form, "paperwork");
        assert_eq!(occurrence.header.scoped_forms[1].kind, "original");
        assert_eq!(occurrence.header.scoped_forms[1].reading, None);
    }
}

fn push_unique_tag(tags: &mut Vec<DictionaryTag>, kind: &str, label: &str) {
    if !tags
        .iter()
        .any(|tag| tag.kind == kind && tag.label == label)
    {
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
