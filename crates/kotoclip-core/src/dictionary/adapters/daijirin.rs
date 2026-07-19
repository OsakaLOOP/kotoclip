use super::{common, AdaptedOccurrence};
use crate::dictionary::html::{parse_fragment, HtmlElement, HtmlNode};
use crate::models::{
    DictionaryAdapterDiagnostics, DictionaryExample, DictionaryForm, DictionaryPronunciation,
    DictionarySection, DictionarySectionItem, DictionarySense, DictionaryTag,
};
use regex::Regex;
use std::sync::LazyLock;

static POS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[（(]((?:動|名|副|形|連|接|感|助|自|他|サ変)[^）)]{0,12})[）)]").unwrap()
});
static HEADER_NOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"【[^】]+】〔([^〕]+)〕").unwrap());
static FORM_SCOPE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"《([^》]+)》").unwrap());
static ORTHOGRAPHY_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^「([^」]+)」は(.+)$").unwrap());
static DERIVATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"━\s*([^（]+?)（([^）]+)）").unwrap());
static TRAILING_NOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"〔([^〕]+)〕\s*$").unwrap());
static LEADING_NOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*〔([^〕]+)〕").unwrap());
static INTERNAL_REFERENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([一二三四五六七八九十]+(?:[①-⑳㋐-㋾])?)に同じ[。．]?$").unwrap()
});

pub fn adapt(
    indexed_headword: &str,
    raw_headword: &str,
    structured_reading: Option<&str>,
    definition: &str,
) -> Vec<AdaptedOccurrence> {
    let root = parse_fragment(definition);
    let indexed_form = indexed_headword
        .split('〖')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(indexed_headword);
    let foreign_origin = foreign_origin(&root, definition);
    let forms = if foreign_origin.is_some() {
        vec![indexed_form.to_string()]
    } else {
        scoped_forms(&root)
    };
    let display_form = choose_display_form(indexed_form, &forms);
    let example_form = if indexed_form.is_empty() {
        display_form.as_str()
    } else {
        indexed_form
    };
    let placeholder_stem = placeholder_stem(&root, example_form);
    let reading = reading_from_headword(raw_headword)
        .or_else(|| first_class_text(&root, "bss"))
        .map(|value| common::normalize_reading(&value))
        .filter(|value| !value.is_empty())
        .or_else(|| structured_reading.map(common::normalize_reading));

    let mut occurrence = AdaptedOccurrence {
        entry_kind: detect_kind(&root, definition).to_string(),
        header: crate::models::DictionaryOccurrenceHeader {
            display_form: display_form.clone(),
            canonical_form: Some(display_form.clone()),
            reading,
            scoped_forms: forms
                .iter()
                .map(|form| DictionaryForm {
                    form: form.clone(),
                    reading: None,
                    kind: if form == &display_form {
                        "canonical"
                    } else {
                        "variant"
                    }
                    .to_string(),
                })
                .chain(
                    (!indexed_form.is_empty() && !forms.iter().any(|form| form == indexed_form))
                        .then(|| DictionaryForm {
                            form: indexed_form.to_string(),
                            reading: None,
                            kind: "indexed".to_string(),
                        }),
                )
                .collect(),
            ..Default::default()
        },
        diagnostics: DictionaryAdapterDiagnostics {
            coverage: "structured".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    occurrence.header.origin = foreign_origin;

    let visible = root.text();
    let mut annotations = Vec::new();
    root.all_by_class("annot", &mut annotations);
    let accent_values = annotations
        .into_iter()
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())
        .fold(Vec::<String>::new(), |mut values, value| {
            if !values.contains(&value) {
                values.push(value);
            }
            values
        });
    if !accent_values.is_empty() {
        occurrence
            .header
            .pronunciations
            .push(DictionaryPronunciation {
                system: "dictionary_accent".to_string(),
                label: "音调".to_string(),
                value: accent_values.join(" / "),
            });
    }
    if let Some(note) = HEADER_NOTE_RE
        .captures(&visible)
        .and_then(|captures| captures.get(1))
        .map(|value| common::normalize_visible_text(value.as_str()))
        .filter(|value| !value.is_empty())
    {
        occurrence.header.short_note = Some(note);
    }
    if occurrence.header.short_note.is_none() {
        if let Some(note) = header_bracket_note(&root) {
            if note.contains("動詞化") || note.contains("形容詞化") || note.contains("名詞化")
            {
                occurrence.header.origin = Some(note);
            } else {
                occurrence.header.short_note = Some(note);
            }
        }
    }
    if occurrence.header.historical_reading.is_none() {
        occurrence.header.historical_reading = header_historical_reading(&root);
    }

    let mut deco = Vec::new();
    root.all_by_class("deco", &mut deco);
    for element in deco {
        let label = common::normalize_visible_text(&element.text());
        if label.is_empty() {
            continue;
        }
        if matches!(label.as_str(), "口語" | "古" | "俗" | "方言" | "専門")
            && !occurrence
                .header
                .usage_tags
                .iter()
                .any(|tag| tag.label == label)
        {
            occurrence
                .header
                .usage_tags
                .push(common::tag("usage", label));
        }
    }

    let mut counter = 0usize;
    occurrence.senses = parse_major_groups(
        &root,
        example_form,
        placeholder_stem.as_deref(),
        &mut counter,
    );
    if let Some(label) = global_pos_label(&root) {
        occurrence.header.pos_tags.push(common::tag("pos", label));
    }
    if let Some((conjugation, historical_reading)) = historical_conjugation(&root) {
        occurrence
            .header
            .usage_tags
            .push(common::tag("usage", "文語"));
        occurrence
            .header
            .pos_tags
            .push(common::tag("pos", format!("文語 {conjugation}")));
        occurrence.header.historical_reading = Some(historical_reading);
    }
    if occurrence.senses.iter().all(|sense| sense.marker.is_none()) {
        for label in header_small_usage_labels(&root) {
            if !occurrence
                .header
                .usage_tags
                .iter()
                .any(|tag| tag.label == label)
            {
                occurrence
                    .header
                    .usage_tags
                    .push(common::tag("grammar", label));
            }
        }
    }
    if occurrence.senses.is_empty() {
        if occurrence.header.pos_tags.is_empty() {
            if let Some(captures) = POS_RE.captures(&visible) {
                if let Some(value) = captures.get(1) {
                    occurrence
                        .header
                        .pos_tags
                        .push(common::tag("pos", value.as_str()));
                }
            }
        }
        let mut containers = Vec::new();
        collect_top_pair_containers(&root, &mut containers);
        for container in containers {
            occurrence.senses.extend(parse_pairs(
                container,
                example_form,
                placeholder_stem.as_deref(),
                &mut counter,
            ));
        }
        if occurrence.senses.is_empty() {
            if let Some(sense) = parse_single_definition(
                &root,
                example_form,
                placeholder_stem.as_deref(),
                &mut counter,
            ) {
                occurrence.senses.push(sense);
            }
        }
    }
    occurrence.sections = collect_deco_sections(&root, example_form, placeholder_stem.as_deref());
    occurrence.links = structural_links(&root);
    if definition.contains("⇒")
        && !occurrence.senses.iter().any(sense_has_substantive_content)
        && (occurrence.links.len()
            + occurrence
                .senses
                .iter()
                .map(|sense| sense.relations.len())
                .sum::<usize>())
            > 0
    {
        occurrence.entry_kind = "redirect".to_string();
        occurrence.diagnostics.coverage = "navigation".to_string();
    }
    if occurrence.senses.is_empty()
        && !occurrence.links.is_empty()
        && matches!(occurrence.entry_kind.as_str(), "lexical" | "unknown")
        && definition.contains('☞')
    {
        occurrence.entry_kind = "navigation".to_string();
        occurrence.diagnostics.coverage = "navigation".to_string();
    } else if occurrence.senses.is_empty() {
        occurrence.diagnostics.coverage = "partial".to_string();
        occurrence
            .diagnostics
            .warnings
            .push("未识别大辞林义项容器，已保留安全降级内容".to_string());
    }
    if (definition.contains("—・") || definition.contains("━・")) && placeholder_stem.is_none()
    {
        occurrence
            .diagnostics
            .warnings
            .push("原文含活用语干占位，但未能从词头读取可靠活用边界".to_string());
    }
    vec![common::finish(occurrence, "daijirin", definition)]
}

fn detect_kind(root: &HtmlElement, definition: &str) -> &'static str {
    let mut elements = Vec::new();
    root.all_elements(&mut elements);
    if elements
        .iter()
        .any(|element| element.attr("type") == Some("漢字"))
    {
        "kanji"
    } else if root.text().contains("姓氏の一") || root.text().contains("姓氏") {
        "surname"
    } else if definition.trim_start().starts_with("@@@LINK=") {
        "redirect"
    } else {
        "lexical"
    }
}

fn scoped_forms(root: &HtmlElement) -> Vec<String> {
    let mut elements = Vec::new();
    root.all_by_name("hy", &mut elements);
    let mut forms = Vec::new();
    for element in elements {
        let value = common::normalize_visible_text(&element.text())
            .replace('▽', "")
            .replace('△', "")
            .replace('▼', "")
            .replace('▲', "")
            .replace('〈', "")
            .replace('〉', "");
        if !value.is_empty() && !forms.contains(&value) {
            forms.push(value);
        }
    }
    forms
}

fn choose_display_form(indexed_headword: &str, forms: &[String]) -> String {
    forms
        .iter()
        .find(|form| form.as_str() == indexed_headword)
        .cloned()
        .or_else(|| forms.first().cloned())
        .unwrap_or_else(|| indexed_headword.to_string())
}

fn foreign_origin(root: &HtmlElement, definition: &str) -> Option<String> {
    if !definition.contains('〖') {
        return None;
    }
    let word = root
        .first_by_class("nk")
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())?;
    let language = root
        .first_by_class("small")
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty());
    Some(match language {
        Some(language) => format!("{language} {word}"),
        None => word,
    })
}

fn reading_from_headword(raw_headword: &str) -> Option<String> {
    let end = raw_headword
        .find(|character| matches!(character, '【' | '〖' | '（' | '('))
        .unwrap_or(raw_headword.len());
    let reading = raw_headword[..end].trim();
    (!reading.is_empty()).then(|| reading.to_string())
}

fn first_class_text(element: &HtmlElement, class: &str) -> Option<String> {
    element
        .first_by_class(class)
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())
}

fn global_pos_label(root: &HtmlElement) -> Option<String> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let paragraph = paragraphs
        .into_iter()
        .find(|paragraph| paragraph.attr("indent") == Some("0"))?;
    let end = paragraph
        .children
        .iter()
        .position(|node| {
            matches!(node, HtmlNode::Element(element) if is_major_deco(element) || has_direct_marker_pair(element))
        })
        .unwrap_or(paragraph.children.len());
    let prefix = HtmlElement {
        name: "prefix".to_string(),
        attrs: Default::default(),
        children: paragraph.children[..end].to_vec(),
    };
    POS_RE
        .captures(&common::normalize_visible_text(&prefix.text()))
        .and_then(|captures| captures.get(1))
        .map(|value| common::normalize_visible_text(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn historical_conjugation(root: &HtmlElement) -> Option<(String, String)> {
    fn visit(element: &HtmlElement) -> Option<(String, String)> {
        for (index, node) in element.children.iter().enumerate() {
            let HtmlNode::Element(candidate) = node else {
                continue;
            };
            if candidate.has_class("deco")
                && common::normalize_visible_text(&candidate.text()) == "文"
            {
                let mut detail = String::new();
                for sibling in &element.children[index + 1..] {
                    match sibling {
                        HtmlNode::Element(element) if element.name == "br" => break,
                        HtmlNode::Element(element) if element.has_class("deco") => break,
                        HtmlNode::Text(text) => detail.push_str(text),
                        HtmlNode::Element(element) => detail.push_str(&element.text()),
                    }
                }
                let detail = common::normalize_visible_text(&detail);
                let mut parts = detail.split_whitespace().collect::<Vec<_>>();
                let reading = parts
                    .pop()
                    .map(common::normalize_reading)
                    .unwrap_or_default();
                let conjugation = common::normalize_visible_text(&parts.join(" "));
                if !conjugation.is_empty() && !reading.is_empty() {
                    return Some((conjugation, reading));
                }
            }
            if let Some(value) = visit(candidate) {
                return Some(value);
            }
        }
        None
    }
    visit(root)
}

fn header_bracket_note(root: &HtmlElement) -> Option<String> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let paragraph = paragraphs
        .into_iter()
        .find(|paragraph| paragraph.attr("indent") == Some("0"))?;
    let end = paragraph
        .children
        .iter()
        .position(
            |node| matches!(node, HtmlNode::Element(element) if has_direct_marker_pair(element)),
        )
        .unwrap_or(paragraph.children.len());
    let prefix = HtmlElement {
        name: "prefix".to_string(),
        attrs: Default::default(),
        children: paragraph.children[..end].to_vec(),
    };
    let text = common::normalize_visible_text(&prefix.text());
    Regex::new(r"〔([^〕]+)〕")
        .expect("valid header note regex")
        .captures(&text)
        .and_then(|captures| captures.get(1))
        .map(|value| common::normalize_visible_text(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn header_historical_reading(root: &HtmlElement) -> Option<String> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let paragraph = paragraphs
        .into_iter()
        .find(|paragraph| paragraph.attr("indent") == Some("0"))?;
    for pair in paragraph.children.windows(2) {
        if matches!(&pair[0], HtmlNode::Element(element) if element.has_class("bss")) {
            if let HtmlNode::Element(element) = &pair[1] {
                if element.has_class("ruby") {
                    let value = common::normalize_reading(&element.text());
                    if !value.is_empty() {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

fn header_small_usage_labels(root: &HtmlElement) -> Vec<String> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let Some(paragraph) = paragraphs
        .into_iter()
        .find(|paragraph| paragraph.attr("indent") == Some("0"))
    else {
        return Vec::new();
    };
    let mut small = Vec::new();
    paragraph.all_by_class("small", &mut small);
    small
        .into_iter()
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| matches!(value.as_str(), "スル" | "タリ"))
        .fold(Vec::new(), |mut labels, label| {
            if !labels.contains(&label) {
                labels.push(label);
            }
            labels
        })
}

fn parse_major_groups(
    root: &HtmlElement,
    display_form: &str,
    placeholder_stem: Option<&str>,
    counter: &mut usize,
) -> Vec<DictionarySense> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let Some(paragraph) = paragraphs
        .into_iter()
        .find(|paragraph| paragraph.attr("indent") == Some("0") || has_major_deco(paragraph))
    else {
        return Vec::new();
    };
    let all_starts = paragraph
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node {
            HtmlNode::Element(element) if is_major_deco(element) => Some(index),
            _ => None,
        })
        .collect::<Vec<_>>();
    let has_invert_groups = all_starts.iter().any(|index| {
        matches!(&paragraph.children[*index], HtmlNode::Element(element) if element.attr("type") == Some("invert-rect"))
    });
    let starts = all_starts
        .into_iter()
        .filter(|index| {
            !has_invert_groups
                || matches!(&paragraph.children[*index], HtmlNode::Element(element) if element.attr("type") == Some("invert-rect"))
        })
        .collect::<Vec<_>>();
    if starts.is_empty() {
        return Vec::new();
    }

    let mut groups = Vec::new();
    for (position, start) in starts.iter().copied().enumerate() {
        let end = starts
            .get(position + 1)
            .copied()
            .unwrap_or(paragraph.children.len());
        let HtmlNode::Element(marker_element) = &paragraph.children[start] else {
            continue;
        };
        *counter += 1;
        let mut group = DictionarySense {
            sense_id: format!("s{counter}"),
            marker: Some(normalize_sense_marker(&common::normalize_visible_text(
                &marker_element.text(),
            ))),
            ..Default::default()
        };
        let raw_segment = HtmlElement {
            name: "segment".to_string(),
            attrs: Default::default(),
            children: paragraph.children[start + 1..end].to_vec(),
        };
        let (segment, pos_label) = strip_leading_pos_line(&raw_segment);
        if let Some(label) = &pos_label {
            group.tags.push(common::tag("pos", label));
        }
        let segment = truncate_at_auxiliary_section(&segment);
        let rect_starts = segment
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                matches!(node, HtmlNode::Element(element) if is_rect_deco(element)).then_some(index)
            })
            .collect::<Vec<_>>();
        let definition_segment = if rect_starts.is_empty() {
            segment.clone()
        } else {
            group.children = parse_rect_groups(
                &segment,
                &rect_starts,
                display_form,
                placeholder_stem,
                counter,
            );
            HtmlElement {
                name: "segment-prefix".to_string(),
                attrs: Default::default(),
                children: segment.children[..rect_starts[0]].to_vec(),
            }
        };
        for label in local_grammar_labels(&definition_segment) {
            if !group
                .tags
                .iter()
                .any(|tag| tag.kind == "grammar" && tag.label == label)
            {
                group.tags.push(common::tag("grammar", label));
            }
        }
        if rect_starts.is_empty() {
            let mut containers = Vec::new();
            collect_top_pair_containers(&segment, &mut containers);
            for container in containers {
                group.children.extend(parse_pairs(
                    container,
                    display_form,
                    placeholder_stem,
                    counter,
                ));
            }
        }
        let (definition, notes) = extract_trailing_notes(body_inline_html(&definition_segment));
        group.notes.extend(notes);
        let relations = sense_links(&definition_segment);
        let (definition, form_tags) = clean_definition(
            common::normalize_visible_text(&definition),
            !relations.is_empty(),
        );
        let definition = expand_placeholders(&definition, display_form, placeholder_stem);
        group.tags.extend(form_tags);
        if let Some(relation) = internal_sense_reference(&definition) {
            group.relations.push(relation);
        } else if !definition.is_empty() {
            group.definitions.push(common::html_text("ja", definition));
        }
        if group.children.is_empty() {
            let mut examples = Vec::new();
            segment.all_by_class("rei", &mut examples);
            for example_group in examples {
                for example in parse_examples(example_group, display_form, placeholder_stem) {
                    group.examples.push(example);
                }
            }
            group.relations.extend(relations);
        }
        promote_parenthetical_heading(&mut group);
        groups.push(group);
    }
    groups
}

fn parse_rect_groups(
    segment: &HtmlElement,
    starts: &[usize],
    display_form: &str,
    placeholder_stem: Option<&str>,
    counter: &mut usize,
) -> Vec<DictionarySense> {
    let mut groups = Vec::new();
    for (position, start) in starts.iter().copied().enumerate() {
        let end = starts
            .get(position + 1)
            .copied()
            .unwrap_or(segment.children.len());
        let HtmlNode::Element(marker_element) = &segment.children[start] else {
            continue;
        };
        *counter += 1;
        let mut group = DictionarySense {
            sense_id: format!("s{counter}"),
            marker: Some(normalize_sense_marker(&common::normalize_visible_text(
                &marker_element.text(),
            ))),
            ..Default::default()
        };
        let child_segment = HtmlElement {
            name: "rect-segment".to_string(),
            attrs: Default::default(),
            children: segment.children[start + 1..end].to_vec(),
        };
        for label in local_grammar_labels(&child_segment) {
            if !group
                .tags
                .iter()
                .any(|tag| tag.kind == "grammar" && tag.label == label)
            {
                group.tags.push(common::tag("grammar", label));
            }
        }
        let mut containers = Vec::new();
        collect_top_pair_containers(&child_segment, &mut containers);
        for container in containers {
            group.children.extend(parse_pairs(
                container,
                display_form,
                placeholder_stem,
                counter,
            ));
        }
        let (definition, notes) = extract_trailing_notes(body_inline_html(&child_segment));
        group.notes.extend(notes);
        let relations = sense_links(&child_segment);
        let (definition, form_tags) = clean_definition(definition, !relations.is_empty());
        let definition = expand_placeholders(&definition, display_form, placeholder_stem);
        group.tags.extend(form_tags);
        if let Some(relation) = internal_sense_reference(&definition) {
            group.relations.push(relation);
        } else if !definition.is_empty() {
            group.definitions.push(common::html_text("ja", definition));
        }
        if group.children.is_empty() {
            let mut examples = Vec::new();
            child_segment.all_by_class("rei", &mut examples);
            for example_group in examples {
                group.examples.extend(parse_examples(
                    example_group,
                    display_form,
                    placeholder_stem,
                ));
            }
            group.relations.extend(relations);
        }
        promote_parenthetical_heading(&mut group);
        groups.push(group);
    }
    groups
}

fn parse_single_definition(
    root: &HtmlElement,
    display_form: &str,
    placeholder_stem: Option<&str>,
    counter: &mut usize,
) -> Option<DictionarySense> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let paragraph = paragraphs
        .into_iter()
        .find(|paragraph| paragraph.attr("indent") == Some("0"))?;
    let body_start = paragraph
        .children
        .iter()
        .position(|node| matches!(node, HtmlNode::Element(element) if element.name == "br"))?
        + 1;
    let raw_segment = HtmlElement {
        name: "segment".to_string(),
        attrs: Default::default(),
        children: paragraph.children[body_start..].to_vec(),
    };
    let (segment, pos_label) = strip_leading_pos_line(&raw_segment);
    let segment = truncate_at_auxiliary_section(&segment);
    if has_major_deco(&segment) || has_direct_marker_pair(&segment) {
        return None;
    }
    let (definition, notes) = extract_trailing_notes(body_inline_html(&segment));
    let mut tags = Vec::new();
    if let Some(label) = pos_label {
        tags.push(common::tag("pos", label));
    }
    let relations = sense_links(&segment);
    let (definition, form_tags) = clean_definition(
        common::normalize_visible_text(&definition),
        !relations.is_empty(),
    );
    let definition = expand_placeholders(&definition, display_form, placeholder_stem);
    tags.extend(form_tags);
    let mut examples = Vec::new();
    let mut example_elements = Vec::new();
    segment.all_by_class("rei", &mut example_elements);
    for example_group in example_elements {
        for example in parse_examples(example_group, display_form, placeholder_stem) {
            examples.push(example);
        }
    }
    let internal_relation = internal_sense_reference(&definition);
    if definition.is_empty()
        && examples.is_empty()
        && relations.is_empty()
        && internal_relation.is_none()
    {
        return None;
    }
    *counter += 1;
    Some(DictionarySense {
        sense_id: format!("s{counter}"),
        definitions: (!definition.is_empty() && internal_relation.is_none())
            .then(|| common::html_text("ja", definition))
            .into_iter()
            .collect(),
        tags,
        examples,
        notes,
        relations: relations.into_iter().chain(internal_relation).collect(),
        ..Default::default()
    })
}

fn sense_has_substantive_content(sense: &DictionarySense) -> bool {
    !sense.definitions.is_empty()
        || !sense.glosses.is_empty()
        || !sense.gloss_groups.is_empty()
        || !sense.examples.is_empty()
        || sense.children.iter().any(sense_has_substantive_content)
}

fn strip_leading_pos_line(segment: &HtmlElement) -> (HtmlElement, Option<String>) {
    let Some(line_end) = segment
        .children
        .iter()
        .position(|node| matches!(node, HtmlNode::Element(element) if element.name == "br"))
    else {
        return (segment.clone(), None);
    };
    let prefix = HtmlElement {
        name: "pos-prefix".to_string(),
        attrs: Default::default(),
        children: segment.children[..line_end].to_vec(),
    };
    let prefix_text = common::normalize_visible_text(&prefix.text());
    let Some(label) = POS_RE
        .captures(&prefix_text)
        .and_then(|captures| captures.get(1))
        .map(|value| common::normalize_visible_text(value.as_str()))
        .filter(|value| !value.is_empty())
    else {
        return (segment.clone(), None);
    };
    (
        HtmlElement {
            name: segment.name.clone(),
            attrs: segment.attrs.clone(),
            children: segment.children[line_end + 1..].to_vec(),
        },
        Some(label),
    )
}

fn truncate_at_auxiliary_section(segment: &HtmlElement) -> HtmlElement {
    let end = segment
        .children
        .iter()
        .position(
            |node| matches!(node, HtmlNode::Element(element) if is_auxiliary_section_deco(element)),
        )
        .unwrap_or(segment.children.len());
    HtmlElement {
        name: segment.name.clone(),
        attrs: segment.attrs.clone(),
        children: segment.children[..end].to_vec(),
    }
}

fn is_auxiliary_section_deco(element: &HtmlElement) -> bool {
    element.has_class("deco")
        && matches!(
            common::normalize_visible_text(&element.text()).as_str(),
            "表記" | "慣用" | "派生" | "可能" | "補足" | "注意"
        )
}

fn has_major_deco(element: &HtmlElement) -> bool {
    common::direct_child_elements(element).any(is_major_deco)
}

fn is_major_deco(element: &HtmlElement) -> bool {
    if !element.has_class("deco") || element.attr("type") == Some("round-rect") {
        return false;
    }
    let marker = common::normalize_visible_text(&element.text());
    marker.chars().count() == 1
        && marker
            .chars()
            .next()
            .is_some_and(|character| matches!(character, '一'..='十'))
}

fn is_rect_deco(element: &HtmlElement) -> bool {
    if !element.has_class("deco") || element.attr("type") != Some("rect") {
        return false;
    }
    let marker = common::normalize_visible_text(&element.text());
    marker.chars().count() == 1
        && marker
            .chars()
            .next()
            .is_some_and(|character| matches!(character, '一'..='十'))
}

fn collect_top_pair_containers<'a>(element: &'a HtmlElement, output: &mut Vec<&'a HtmlElement>) {
    if has_direct_marker_pair(element) {
        output.push(element);
        return;
    }
    for child in common::direct_child_elements(element) {
        if child.has_class("rei") {
            continue;
        }
        collect_top_pair_containers(child, output);
    }
}

fn has_direct_marker_pair(element: &HtmlElement) -> bool {
    let children = common::direct_child_elements(element).collect::<Vec<_>>();
    children.windows(2).any(|pair| {
        marker_text(pair[0]).is_some()
            && (pair[1].has_class("lefta") || pair[1].has_class("leftb") || pair[1].name == "div")
    })
}

fn parse_pairs(
    container: &HtmlElement,
    display_form: &str,
    placeholder_stem: Option<&str>,
    counter: &mut usize,
) -> Vec<DictionarySense> {
    let children = common::direct_child_elements(container).collect::<Vec<_>>();
    let mut senses = Vec::new();
    let mut index = 0usize;
    while index + 1 < children.len() {
        let Some(marker) = marker_text(children[index]) else {
            index += 1;
            continue;
        };
        let body = children[index + 1];
        if !(body.has_class("lefta") || body.has_class("leftb") || body.name == "div") {
            index += 1;
            continue;
        }
        *counter += 1;
        let mut sense = DictionarySense {
            sense_id: format!("s{counter}"),
            marker: Some(marker),
            relations: sense_links_scoped(body),
            ..Default::default()
        };
        let (raw_definition, notes) = extract_trailing_notes(body_inline_html(body));
        sense.notes.extend(notes);
        let (definition, form_tags) = clean_definition(raw_definition, !sense.relations.is_empty());
        let definition = expand_placeholders(&definition, display_form, placeholder_stem);
        sense.tags.extend(form_tags);
        if let Some(relation) = internal_sense_reference(&definition) {
            sense.relations.push(relation);
        } else if !definition.trim().is_empty() {
            sense.definitions.push(common::html_text("ja", definition));
        }
        let mut examples = Vec::new();
        collect_scoped_by_class(body, "rei", &mut examples);
        for example_group in examples {
            for example in parse_examples(example_group, display_form, placeholder_stem) {
                sense.examples.push(example);
            }
        }
        let mut nested = Vec::new();
        for child in common::direct_child_elements(body) {
            collect_top_pair_containers(child, &mut nested);
        }
        for nested_container in nested {
            sense.children.extend(parse_pairs(
                nested_container,
                display_form,
                placeholder_stem,
                counter,
            ));
        }
        promote_parenthetical_heading(&mut sense);
        senses.push(sense);
        index += 2;
    }
    senses
}

fn marker_text(element: &HtmlElement) -> Option<String> {
    if element.has_class("deco") {
        return None;
    }
    let value = common::normalize_visible_text(&element.text());
    let value = value.trim().trim_end_matches(['.', '．']).trim();
    if element.has_class("no") || is_marker(value) {
        Some(normalize_sense_marker(value))
    } else {
        None
    }
}

fn normalize_sense_marker(value: &str) -> String {
    let mut chars = value.chars();
    let Some(character) = chars.next() else {
        return String::new();
    };
    if chars.next().is_some() {
        return value.to_string();
    }
    let number = match character {
        '①'..='⑳' => character as u32 - '①' as u32 + 1,
        '㉑'..='㉟' => character as u32 - '㉑' as u32 + 21,
        '㊱'..='㊿' => character as u32 - '㊱' as u32 + 36,
        _ => return value.to_string(),
    };
    number.to_string()
}

fn is_marker(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }
    matches!(first, '①'..='⑳' | '㉑'..='㉟' | '㊱'..='㊿' | '㋐'..='㋾' | '一'..='十')
}

fn body_inline_html(element: &HtmlElement) -> String {
    let mut output = String::new();
    render_body_nodes(&element.children, &mut output);
    let normalized = common::normalize_visible_text(&output.replace("<br>", " "));
    normalized
        .replace("&lt;ruby&gt;", "<ruby>")
        .replace("&lt;/ruby&gt;", "</ruby>")
        .replace("&lt;rt&gt;", "<rt>")
        .replace("&lt;/rt&gt;", "</rt>")
        .replace("&amp;", "&")
}

fn render_body_nodes(nodes: &[HtmlNode], output: &mut String) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => output.push_str(&common::escape_html(text)),
            HtmlNode::Element(element) => {
                if element.has_class("rei")
                    || element.name == "a"
                    || element.has_class("annot")
                    || has_direct_marker_pair(element)
                {
                    continue;
                }
                if element.has_class("ruby") {
                    wrap_trailing_kanji(output, &common::normalize_visible_text(&element.text()));
                    continue;
                }
                if element.name == "br" {
                    output.push(' ');
                    continue;
                }
                if element.name == "small" || element.has_class("small") {
                    let label = common::normalize_visible_text(&element.text());
                    if is_local_grammar_label(&label) {
                        continue;
                    }
                    output.push_str("<small>");
                    render_body_nodes(&element.children, output);
                    output.push_str("</small>");
                    continue;
                }
                render_body_nodes(&element.children, output);
            }
        }
    }
}

fn local_grammar_labels(element: &HtmlElement) -> Vec<String> {
    let mut small = Vec::new();
    element.all_by_class("small", &mut small);
    small
        .into_iter()
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| is_local_grammar_label(value))
        .fold(Vec::new(), |mut labels, label| {
            if !labels.contains(&label) {
                labels.push(label);
            }
            labels
        })
}

fn is_local_grammar_label(value: &str) -> bool {
    matches!(value, "スル" | "タリ")
}

fn internal_sense_reference(definition: &str) -> Option<crate::models::DictionaryLink> {
    let plain =
        common::normalize_visible_text(&definition.replace("<small>", "").replace("</small>", ""));
    let plain = plain
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let label = INTERNAL_REFERENCE_RE
        .captures(&plain)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())?;
    Some(crate::models::DictionaryLink {
        target: format!("sense-marker:{label}"),
        label,
        relation: "internal_reference".to_string(),
    })
}

fn wrap_trailing_kanji(output: &mut String, reading: &str) {
    if reading.is_empty() {
        return;
    }
    let mut start = output.len();
    for (index, character) in output.char_indices().rev() {
        if is_kanji(character) {
            start = index;
        } else {
            break;
        }
    }
    if start == output.len() {
        output.push_str("（");
        output.push_str(&common::escape_html(reading));
        output.push('）');
        return;
    }
    let base = output[start..].to_string();
    output.truncate(start);
    output.push_str("<ruby>");
    output.push_str(&base);
    output.push_str("<rt>");
    output.push_str(&common::escape_html(reading));
    output.push_str("</rt></ruby>");
}

fn is_kanji(character: char) -> bool {
    matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

fn parse_examples(
    element: &HtmlElement,
    display_form: &str,
    placeholder_stem: Option<&str>,
) -> Vec<DictionaryExample> {
    let mut rendered = String::new();
    render_example_nodes(&element.children, &mut rendered);
    let normalized = common::normalize_visible_text(&rendered)
        .replace("&lt;ruby&gt;", "<ruby>")
        .replace("&lt;/ruby&gt;", "</ruby>")
        .replace("&lt;rt&gt;", "<rt>")
        .replace("&lt;/rt&gt;", "</rt>")
        .replace("&lt;small&gt;", "<small>")
        .replace("&lt;/small&gt;", "</small>")
        .replace("&amp;", "&");
    let mut values = Vec::new();
    let mut rest = normalized.as_str();
    while let Some(start) = rest.find('「') {
        let after = &rest[start + '「'.len_utf8()..];
        let Some(end) = after.find('」') else {
            break;
        };
        let value = expand_placeholders(&after[..end], display_form, placeholder_stem);
        let value = common::normalize_visible_text(&value);
        if !value.is_empty() {
            values.push(DictionaryExample {
                source: common::html_text("ja", value),
                ..Default::default()
            });
        }
        rest = &after[end + '」'.len_utf8()..];
    }
    if values.is_empty() && !normalized.is_empty() {
        values.push(DictionaryExample {
            source: common::html_text(
                "ja",
                expand_placeholders(&normalized, display_form, placeholder_stem),
            ),
            ..Default::default()
        });
    }
    values
}

fn placeholder_stem(root: &HtmlElement, display_form: &str) -> Option<String> {
    let source = first_class_text(root, "bss")?;
    let (_, inflection_tail) = source.rsplit_once('・')?;
    let inflection_tail = common::normalize_visible_text(inflection_tail);
    if inflection_tail.is_empty() {
        return None;
    }
    display_form
        .strip_suffix(&inflection_tail)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn expand_placeholders(value: &str, display_form: &str, placeholder_stem: Option<&str>) -> String {
    let stem = placeholder_stem.unwrap_or(display_form);
    value
        .replace("━・", stem)
        .replace("—・", stem)
        .replace("―・", stem)
        .replace('━', display_form)
        .replace('—', display_form)
        .replace('―', display_form)
}

fn render_example_nodes(nodes: &[HtmlNode], output: &mut String) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => output.push_str(&common::escape_html(text)),
            HtmlNode::Element(element) if element.has_class("ruby") => {
                wrap_trailing_kanji(output, &common::normalize_visible_text(&element.text()));
            }
            HtmlNode::Element(element) if element.name == "small" || element.has_class("small") => {
                output.push_str("<small>");
                render_example_nodes(&element.children, output);
                output.push_str("</small>");
            }
            HtmlNode::Element(element) => render_example_nodes(&element.children, output),
        }
    }
}

fn clean_definition(mut definition: String, has_relations: bool) -> (String, Vec<DictionaryTag>) {
    let form_scopes = FORM_SCOPE_RE
        .captures_iter(&definition)
        .filter_map(|captures| captures.get(1))
        .map(|value| common::normalize_visible_text(value.as_str()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    definition = FORM_SCOPE_RE.replace_all(&definition, "").to_string();
    if has_relations {
        for marker in ['⇔', '⇒', '→', '☞'] {
            definition = definition.replace(marker, "");
        }
    }
    let definition = common::normalize_visible_text(&definition);
    let mut tags = Vec::new();
    for scope in form_scopes {
        for form in scope.split(['・', '／', '/']) {
            let form = common::normalize_visible_text(form);
            if !form.is_empty()
                && !tags
                    .iter()
                    .any(|tag: &DictionaryTag| tag.kind == "form" && tag.label == form)
            {
                tags.push(common::tag("form", form));
            }
        }
    }
    (definition, tags)
}

fn extract_trailing_notes(mut definition: String) -> (String, Vec<crate::models::DictionaryText>) {
    let mut notes = Vec::new();
    loop {
        let Some(captures) = TRAILING_NOTE_RE.captures(&definition) else {
            break;
        };
        let Some(whole) = captures.get(0) else {
            break;
        };
        let note = captures
            .get(1)
            .map(|value| common::normalize_visible_text(value.as_str()))
            .unwrap_or_default();
        definition.truncate(whole.start());
        definition = definition.trim_end().to_string();
        if !note.is_empty() {
            notes.insert(0, common::html_text("ja", note));
        }
    }
    if let Some(captures) = LEADING_NOTE_RE.captures(&definition) {
        if let (Some(whole), Some(note)) = (captures.get(0), captures.get(1)) {
            let note = common::normalize_visible_text(note.as_str());
            let rest = definition[whole.end()..].trim_start().to_string();
            definition = rest;
            if !note.is_empty() {
                notes.insert(0, common::html_text("ja", note));
            }
        }
    }
    (definition, notes)
}

fn promote_parenthetical_heading(sense: &mut DictionarySense) {
    if sense.children.is_empty() || sense.definitions.len() != 1 || sense.heading.is_some() {
        return;
    }
    let value = sense.definitions[0].html.trim();
    let is_parenthetical = (value.starts_with('（') && value.ends_with('）'))
        || (value.starts_with('(') && value.ends_with(')'));
    if is_parenthetical && !value.contains('。') {
        sense.heading = Some(value.to_string());
        sense.definitions.clear();
    }
}

fn sense_links(element: &HtmlElement) -> Vec<crate::models::DictionaryLink> {
    let text = element.text();
    let relation = if text.contains('⇔') || text.contains("対義") {
        "antonym"
    } else if text.contains("類語") || text.contains("同義") {
        "synonym"
    } else {
        "reference"
    };
    common::extract_links(element, relation)
}

fn sense_links_scoped(element: &HtmlElement) -> Vec<crate::models::DictionaryLink> {
    let mut text = String::new();
    collect_scoped_text(&element.children, &mut text);
    let relation = if text.contains('⇔') || text.contains("対義") {
        "antonym"
    } else if text.contains("類語") || text.contains("同義") {
        "synonym"
    } else {
        "reference"
    };
    let mut anchors = Vec::new();
    collect_scoped_anchors(&element.children, &mut anchors);
    let mut links = Vec::new();
    for anchor in anchors {
        let Some(target) = anchor
            .attr("href")
            .and_then(|href| href.strip_prefix("entry://"))
            .map(str::trim)
            .filter(|target| !target.is_empty())
        else {
            continue;
        };
        if links
            .iter()
            .any(|link: &crate::models::DictionaryLink| link.target == target)
        {
            continue;
        }
        let label = common::normalize_visible_text(&anchor.text());
        links.push(crate::models::DictionaryLink {
            target: target.to_string(),
            label: if label.is_empty() {
                target.to_string()
            } else {
                label
            },
            relation: relation.to_string(),
        });
    }
    links
}

fn collect_scoped_by_class<'a>(
    element: &'a HtmlElement,
    class: &str,
    output: &mut Vec<&'a HtmlElement>,
) {
    for child in common::direct_child_elements(element) {
        if has_direct_marker_pair(child) {
            continue;
        }
        if child.has_class(class) {
            output.push(child);
        } else {
            collect_scoped_by_class(child, class, output);
        }
    }
}

fn collect_scoped_text(nodes: &[HtmlNode], output: &mut String) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => output.push_str(text),
            HtmlNode::Element(element) if has_direct_marker_pair(element) => {}
            HtmlNode::Element(element) => collect_scoped_text(&element.children, output),
        }
    }
}

fn collect_scoped_anchors<'a>(nodes: &'a [HtmlNode], output: &mut Vec<&'a HtmlElement>) {
    for node in nodes {
        let HtmlNode::Element(element) = node else {
            continue;
        };
        if has_direct_marker_pair(element) {
            continue;
        }
        if element.name == "a" {
            output.push(element);
        } else {
            collect_scoped_anchors(&element.children, output);
        }
    }
}

fn structural_links(root: &HtmlElement) -> Vec<crate::models::DictionaryLink> {
    let mut paragraphs = Vec::new();
    root.all_by_name("p", &mut paragraphs);
    let mut links = Vec::new();
    for paragraph in paragraphs {
        let text = paragraph.text();
        if text.contains('☞') {
            for link in navigation_candidates(paragraph) {
                if !links
                    .iter()
                    .any(|item: &crate::models::DictionaryLink| item.target == link.target)
                {
                    links.push(link);
                }
            }
            continue;
        }
        let has_structural_label = text.contains("〈親項目〉")
            || text.contains("〈子項目〉")
            || text.contains("〈句項目〉");
        if paragraph.attr("indent") == Some("0") && !has_structural_label {
            continue;
        }
        let relation = if text.contains("〈親項目〉") {
            "parent"
        } else if text.contains("〈子項目〉") {
            "child"
        } else if text.contains("〈句項目〉") {
            "phrase"
        } else if text.contains('☞') {
            "candidate"
        } else {
            "related"
        };
        for link in common::extract_links(paragraph, relation) {
            if !links.iter().any(|item: &crate::models::DictionaryLink| {
                item.target == link.target && item.relation == link.relation
            }) {
                links.push(link);
            }
        }
    }
    links
}

fn navigation_candidates(paragraph: &HtmlElement) -> Vec<crate::models::DictionaryLink> {
    let mut links = Vec::new();
    let mut line_has_candidate = false;
    let mut line_is_navigation = false;
    for node in &paragraph.children {
        match node {
            HtmlNode::Text(text) => {
                if text.contains('☞') {
                    line_is_navigation = true;
                    line_has_candidate = false;
                }
            }
            HtmlNode::Element(element) if element.name == "br" => {
                line_is_navigation = false;
                line_has_candidate = false;
            }
            HtmlNode::Element(element)
                if element.name == "a" && line_is_navigation && !line_has_candidate =>
            {
                if let Some(target) = element
                    .attr("href")
                    .and_then(|href| href.strip_prefix("entry://"))
                {
                    links.push(crate::models::DictionaryLink {
                        target: target.to_string(),
                        label: common::normalize_visible_text(&element.text()),
                        relation: "candidate".to_string(),
                    });
                    line_has_candidate = true;
                }
            }
            _ => {}
        }
    }
    links
}

fn collect_deco_sections(
    root: &HtmlElement,
    example_form: &str,
    placeholder_stem: Option<&str>,
) -> Vec<DictionarySection> {
    let mut sections = Vec::new();
    collect_deco_sections_in(root, example_form, placeholder_stem, &mut sections);
    sections
}

fn collect_deco_sections_in(
    element: &HtmlElement,
    example_form: &str,
    placeholder_stem: Option<&str>,
    output: &mut Vec<DictionarySection>,
) {
    for (index, child) in element.children.iter().enumerate() {
        let HtmlNode::Element(deco) = child else {
            continue;
        };
        if deco.has_class("deco") {
            let label = common::normalize_visible_text(&deco.text());
            if matches!(
                label.as_str(),
                "表記" | "慣用" | "派生" | "可能" | "補足" | "注意"
            ) {
                let end = element.children[index + 1..]
                    .iter()
                    .position(|sibling| {
                        matches!(sibling, HtmlNode::Element(element) if element.has_class("deco"))
                    })
                    .map(|offset| index + 1 + offset)
                    .unwrap_or(element.children.len());
                let lines = render_section_lines(&element.children[index + 1..end]);
                if let Some(section) =
                    build_deco_section(&label, &lines, example_form, placeholder_stem)
                {
                    output.push(section);
                }
            }
        }
        collect_deco_sections_in(deco, example_form, placeholder_stem, output);
    }
}

fn render_section_lines(nodes: &[HtmlNode]) -> Vec<String> {
    let mut rendered = String::new();
    render_section_nodes(nodes, &mut rendered);
    rendered
        .replace("&lt;ruby&gt;", "<ruby>")
        .replace("&lt;/ruby&gt;", "</ruby>")
        .replace("&lt;rt&gt;", "<rt>")
        .replace("&lt;/rt&gt;", "</rt>")
        .replace("&lt;small&gt;", "<small>")
        .replace("&lt;/small&gt;", "</small>")
        .replace("&amp;", "&")
        .lines()
        .map(common::normalize_visible_text)
        .filter(|line| !line.is_empty())
        .collect()
}

fn render_section_nodes(nodes: &[HtmlNode], output: &mut String) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => output.push_str(&common::escape_html(text)),
            HtmlNode::Element(element) if element.name == "br" => output.push('\n'),
            HtmlNode::Element(element) if element.has_class("ruby") => {
                wrap_trailing_kanji(output, &common::normalize_visible_text(&element.text()));
            }
            HtmlNode::Element(element) if element.name == "small" || element.has_class("small") => {
                output.push_str("<small>");
                render_section_nodes(&element.children, output);
                output.push_str("</small>");
            }
            HtmlNode::Element(element) => render_section_nodes(&element.children, output),
        }
    }
}

fn build_deco_section(
    label: &str,
    lines: &[String],
    example_form: &str,
    placeholder_stem: Option<&str>,
) -> Option<DictionarySection> {
    match label {
        "慣用" => {
            let items = lines
                .iter()
                .flat_map(|line| line.split('・'))
                .map(|phrase| phrase.replace('━', example_form).replace('—', example_form))
                .map(|phrase| common::normalize_visible_text(&phrase))
                .filter(|phrase| !phrase.is_empty())
                .map(|phrase| DictionarySectionItem {
                    content: vec![common::html_text("ja", phrase)],
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            (!items.is_empty()).then(|| DictionarySection {
                kind: "idioms".to_string(),
                label: Some("惯用表达".to_string()),
                items,
            })
        }
        "表記" => {
            let mut items = Vec::new();
            for line in lines {
                if !line.contains('「') && line.contains(['（', '(']) {
                    continue;
                }
                if let Some(captures) = ORTHOGRAPHY_LINE_RE.captures(line) {
                    let form = captures
                        .get(1)
                        .map(|value| common::normalize_visible_text(value.as_str()));
                    let detail = captures
                        .get(2)
                        .map(|value| common::normalize_visible_text(value.as_str()))
                        .unwrap_or_default();
                    items.push(DictionarySectionItem {
                        label: form,
                        content: (!detail.is_empty())
                            .then(|| common::html_text("ja", detail))
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    });
                } else {
                    items.push(DictionarySectionItem {
                        content: vec![common::html_text("ja", line)],
                        ..Default::default()
                    });
                }
            }
            (!items.is_empty()).then(|| DictionarySection {
                kind: "orthography".to_string(),
                label: Some("表记用法".to_string()),
                items,
            })
        }
        "可能" => {
            let items = lines
                .iter()
                .map(|line| DictionarySectionItem {
                    content: vec![common::html_text("ja", line)],
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            (!items.is_empty()).then(|| DictionarySection {
                kind: "conjugation".to_string(),
                label: Some("可能形".to_string()),
                items,
            })
        }
        "派生" => {
            let stem = placeholder_stem.unwrap_or(example_form);
            let mut items = Vec::new();
            for line in lines {
                for captures in DERIVATION_RE.captures_iter(line) {
                    let suffix = captures
                        .get(1)
                        .map(|value| {
                            common::normalize_visible_text(value.as_str())
                                .replace('・', "")
                                .replace(' ', "")
                        })
                        .unwrap_or_default();
                    let pos = captures
                        .get(2)
                        .map(|value| {
                            common::normalize_visible_text(value.as_str())
                                .replace("<small>", "")
                                .replace("</small>", "")
                        })
                        .unwrap_or_default();
                    if suffix.is_empty() {
                        continue;
                    }
                    items.push(DictionarySectionItem {
                        label: Some(format!("{stem}{suffix}")),
                        tags: (!pos.is_empty())
                            .then(|| common::tag("pos", pos))
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    });
                }
            }
            (!items.is_empty()).then(|| DictionarySection {
                kind: "derivations".to_string(),
                label: Some("派生词".to_string()),
                items,
            })
        }
        _ => {
            let items = lines
                .iter()
                .map(|line| DictionarySectionItem {
                    content: vec![common::html_text("ja", line)],
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            (!items.is_empty()).then(|| DictionarySection {
                kind: "notes".to_string(),
                label: Some(label.to_string()),
                items,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_pos_html_and_auxiliary_sections_from_major_definition() {
        let definition = r#"<p indent="0"><span class="bss">た・つ</span>【<hy>立つ</hy>】<br/><span class="deco" type="invert-rect">一</span>（動<span class="small">タ</span>五）<br/><div><div class="no">①</div><div class="lefta">直立する。<span class="rei">「山に—・つ」</span></div></div><span class="deco" type="round-rect">可能</span>たてる<br/><span class="deco" type="round-rect">表記</span>「立つ」は直立の意。</p>"#;
        let occurrence = adapt("立つ", "たつ【立つ】", Some("たつ"), definition)
            .into_iter()
            .next()
            .expect("应生成 occurrence");
        assert_eq!(occurrence.senses.len(), 1);
        assert_eq!(occurrence.senses[0].tags[0].label, "動タ五");
        assert!(occurrence.senses[0]
            .definitions
            .iter()
            .all(|definition| !definition.html.contains("small")
                && !definition.html.contains("可能")));
        assert!(occurrence
            .sections
            .iter()
            .any(|section| section.kind == "conjugation"));
        assert!(occurrence
            .sections
            .iter()
            .any(|section| section.kind == "orthography"));
    }

    #[test]
    fn keeps_marker_pair_definition_without_indent_attribute() {
        let definition = r#"<p><span class="bss">こ</span>【<hy>子</hy>】<br><div><div class="no">①</div><div class="lefta">子供。⇔<a href="entry://親">親</a>・<a href="entry://祖">祖</a>。</div></div></p>"#;
        let occurrence = adapt("子", "こ【子】", Some("こ"), definition)
            .into_iter()
            .next()
            .expect("应生成 occurrence");
        assert!(
            occurrence.definition_html.contains("子供"),
            "definition_html={}",
            occurrence.definition_html
        );
        assert_eq!(occurrence.senses[0].marker.as_deref(), Some("1"));
    }

    #[test]
    fn normalizes_enclosed_numeric_markers_without_changing_other_levels() {
        assert_eq!(normalize_sense_marker("①"), "1");
        assert_eq!(normalize_sense_marker("⑳"), "20");
        assert_eq!(normalize_sense_marker("㉑"), "21");
        assert_eq!(normalize_sense_marker("㊿"), "50");
        assert_eq!(normalize_sense_marker("一"), "一");
        assert_eq!(normalize_sense_marker("㋐"), "㋐");
    }
}
