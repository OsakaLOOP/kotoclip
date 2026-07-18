use super::{common, AdaptedOccurrence};
use crate::dictionary::html::{parse_fragment, HtmlElement, HtmlNode};
use crate::models::{
    DictionaryAdapterDiagnostics, DictionaryExample, DictionaryForm, DictionarySection,
    DictionarySectionItem, DictionarySense, DictionaryText,
};

pub fn adapt(
    indexed_headword: &str,
    raw_headword: &str,
    structured_reading: Option<&str>,
    definition: &str,
) -> Vec<AdaptedOccurrence> {
    let root = parse_fragment(definition);
    let display_form = root
        .first_by_class("hw_hyoki")
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| indexed_headword.to_string());
    let suffix = root
        .first_by_class("mj_katsuyogobi")
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty());
    let base_reading = root
        .first_by_class("hw_midashi")
        .map(HtmlElement::text)
        .map(|value| common::normalize_reading(&value))
        .filter(|value| !value.is_empty())
        .or_else(|| structured_reading.map(common::normalize_reading));
    let reading = match (base_reading.clone(), suffix.as_deref()) {
        (Some(reading), Some(suffix)) if display_form.ends_with(suffix) && !reading.ends_with(suffix) => {
            Some(format!("{reading}{suffix}"))
        }
        (reading, _) => reading,
    };
    let mut scoped_forms = vec![DictionaryForm {
        form: display_form.clone(),
        reading: reading.clone(),
        kind: "canonical".to_string(),
    }];
    if let Some(stem) = suffix
        .as_deref()
        .and_then(|suffix| display_form.strip_suffix(suffix))
        .filter(|stem| !stem.is_empty())
    {
        scoped_forms.push(DictionaryForm {
            form: stem.to_string(),
            reading: base_reading,
            kind: "stem".to_string(),
        });
    }

    let mut occurrence = AdaptedOccurrence {
        entry_kind: "lexical".to_string(),
        header: crate::models::DictionaryOccurrenceHeader {
            display_form: display_form.clone(),
            canonical_form: Some(display_form.clone()),
            reading,
            origin: foreign_origin(raw_headword),
            scoped_forms,
            ..Default::default()
        },
        diagnostics: DictionaryAdapterDiagnostics {
            coverage: "structured".to_string(),
            omitted: vec!["拼音与英文对应默认从主释义及例句省略".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };

    let mut sense_elements = Vec::new();
    root.all_by_class("mean_gogi", &mut sense_elements);
    for (index, element) in sense_elements.into_iter().enumerate() {
        let marker = class_text(element, "kg_gogi");
        let heading = class_text(element, "mean_kubun")
            .or_else(|| class_text(element, "mean_eiyaku_kubun"));
        let glosses = crown_glosses(element);
        let mut examples = Vec::new();
        let mut example_elements = Vec::new();
        element.all_by_class("mean_yoreiyaku", &mut example_elements);
        for example_element in example_elements {
            let source = example_element.first_by_class("mean_yorei");
            let translation = example_element
                .first_by_class("mean_reiyaku")
                .map(|element| element.text_excluding_classes(&["pinyin_box"]))
                .map(|value| common::normalize_visible_text(&value))
                .filter(|value| !value.is_empty());
            if let Some(example) = source.and_then(|source| {
                crown_example(source, translation.as_deref())
            }) {
                examples.push(example);
            }
        }

        let mut sense = DictionarySense {
            sense_id: format!("s{}", index + 1),
            marker,
            heading,
            glosses,
            examples,
            relations: common::extract_links(element, "reference"),
            ..Default::default()
        };
        if sense.glosses.is_empty() {
            let residual = common::normalize_visible_text(&element.text_excluding_classes(&[
                "kg_gogi",
                "group_yoreiyaku",
                "pinyin_box",
                "eiyaku_box",
                "mean_kubun",
                "mean_eiyaku_kubun",
            ]));
            if !residual.is_empty() {
                sense.definitions.push(common::text("ja", residual));
            }
        }
        occurrence.senses.push(sense);
    }

    if occurrence.senses.iter().any(|sense| {
        sense
            .heading
            .as_deref()
            .is_some_and(|heading| heading.contains("牛の声") || heading.contains("鳴き声"))
    }) {
        occurrence.entry_kind = "onomatopoeia".to_string();
        occurrence
            .header
            .usage_tags
            .push(common::tag("usage", "拟声"));
    }
    let mut english = Vec::new();
    root.all_by_class("mean_eiyaku", &mut english);
    let is_explicit_prefix = english.iter().any(|element| {
        common::normalize_visible_text(&element.text()).ends_with('-')
    }) && occurrence
        .senses
        .iter()
        .all(|sense| sense.glosses.is_empty() && sense.definitions.is_empty());
    if is_explicit_prefix {
        occurrence.entry_kind = "prefix".to_string();
        occurrence
            .header
            .usage_tags
            .push(common::tag("grammar", "接头成分"));
        for form in &mut occurrence.header.scoped_forms {
            form.kind = "prefix".to_string();
        }
        for sense in &mut occurrence.senses {
            if sense.heading.is_none() {
                sense.heading = Some("接头成分".to_string());
            }
        }
    }

    for (class, kind, label, item_class, label_class) in [
        ("group_hukugo", "compounds", "复合词", "item_sub_hukugo", "shw_hukugo"),
        ("group_kanyo", "idioms", "惯用语", "item_sub_kanyo", "midashi_sub_kanyo"),
        ("group_kotowaza", "proverbs", "谚语", "item_sub_kotowaza", "shw_kotowaza"),
    ] {
        let section = parse_section(&root, class, kind, label, item_class, label_class);
        if let Some(section) = section {
            occurrence.sections.push(section);
        }
    }

    let mut columns = Vec::new();
    root.all_by_class("item_sub_column", &mut columns);
    if !columns.is_empty() {
        occurrence.sections.push(DictionarySection {
            kind: "notes".to_string(),
            label: Some("补充说明".to_string()),
            items: columns
                .into_iter()
                .filter_map(|column| {
                    let content = common::normalize_visible_text(
                        &column.text_excluding_classes(&["pinyin_box"]),
                    );
                    (!content.is_empty()).then(|| DictionarySectionItem {
                        content: vec![common::text("zh-CN", content)],
                        ..Default::default()
                    })
                })
                .collect(),
        });
    }

    let all_text = root.text();
    if all_text.contains("姓氏") || all_text.contains("姓の一") {
        occurrence.entry_kind = "surname".to_string();
        occurrence.header.usage_tags.push(common::tag("proper", "姓氏"));
    }
    occurrence.links = common::extract_links(&root, "reference");
    if occurrence.senses.is_empty() && occurrence.sections.is_empty() {
        occurrence.diagnostics.coverage = "partial".to_string();
        occurrence
            .diagnostics
            .warnings
            .push("未识别 Crown 主义项结构，已保留安全降级内容".to_string());
    }
    vec![common::finish(occurrence, "crown", definition)]
}

fn class_text(element: &HtmlElement, class: &str) -> Option<String> {
    element
        .first_by_class(class)
        .map(HtmlElement::text)
        .map(|value| common::normalize_visible_text(&value))
        .filter(|value| !value.is_empty())
}

fn crown_glosses(element: &HtmlElement) -> Vec<DictionaryText> {
    let mut boxes = Vec::new();
    element.all_by_class("yakugo_sub_box", &mut boxes);
    let mut glosses = Vec::new();
    for box_element in boxes {
        let qualifier = class_text(box_element, "mean_shokubun")
            .map(|value| {
                value
                    .trim_matches(|character| matches!(character, '｟' | '｠' | '（' | '）'))
                    .to_string()
            })
            .filter(|value| !value.is_empty());
        let mut values = Vec::new();
        box_element.all_by_class("mean_yakugo", &mut values);
        for value in values {
            let value = common::normalize_visible_text(
                &value.text_excluding_classes(&["pinyin_box"]),
            );
            if value.is_empty() {
                continue;
            }
            let item = qualifier
                .as_deref()
                .map(|qualifier| common::qualified_text("zh-CN", qualifier, &value))
                .unwrap_or_else(|| common::text("zh-CN", &value));
            if !glosses.iter().any(|existing: &DictionaryText| {
                existing.qualifier == item.qualifier && existing.html == item.html
            }) {
                glosses.push(item);
            }
        }
    }
    if glosses.is_empty() {
        let mut values = Vec::new();
        element.all_by_class("mean_yakugo", &mut values);
        for value in values {
            let value = common::normalize_visible_text(
                &value.text_excluding_classes(&["pinyin_box"]),
            );
            if !value.is_empty() {
                glosses.push(common::text("zh-CN", value));
            }
        }
    }
    glosses
}

fn parse_section(
    root: &HtmlElement,
    class: &str,
    kind: &str,
    label: &str,
    item_class: &str,
    label_class: &str,
) -> Option<DictionarySection> {
    let mut groups = Vec::new();
    root.all_by_class(class, &mut groups);
    let mut items = Vec::new();
    for group in groups {
        let mut item_elements = Vec::new();
        group.all_by_class(item_class, &mut item_elements);
        if item_elements.is_empty() {
            item_elements.push(group);
        }
        for item in item_elements {
            let item_label = class_text(item, label_class);
            let mut gloss_elements = Vec::new();
            item.all_by_class("mean_yakugo", &mut gloss_elements);
            let content = gloss_elements
                .into_iter()
                .filter_map(|gloss| {
                    let value = common::normalize_visible_text(
                        &gloss.text_excluding_classes(&["pinyin_box"]),
                    );
                    (!value.is_empty()).then(|| common::text("zh-CN", value))
                })
                .collect::<Vec<_>>();
            let mut examples = Vec::new();
            let mut example_elements = Vec::new();
            item.all_by_class("mean_yoreiyaku", &mut example_elements);
            for example_element in example_elements {
                let source = example_element.first_by_class("mean_yorei");
                let translation = example_element
                    .first_by_class("mean_reiyaku")
                    .map(|element| element.text_excluding_classes(&["pinyin_box"]))
                    .map(|value| common::normalize_visible_text(&value))
                    .filter(|value| !value.is_empty());
                if let Some(example) = source.and_then(|source| {
                    crown_example(source, translation.as_deref())
                }) {
                    examples.push(example);
                }
            }
            let fallback_content = if content.is_empty() && examples.is_empty() {
                let value = common::normalize_visible_text(&item.text_excluding_classes(&[
                    label_class,
                    "pinyin_box",
                    "eiyaku_box",
                ]));
                (!value.is_empty())
                    .then(|| vec![common::text("zh-CN", value)])
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            if item_label.is_none()
                && content.is_empty()
                && fallback_content.is_empty()
                && examples.is_empty()
            {
                continue;
            }
            items.push(DictionarySectionItem {
                label: item_label,
                content: if content.is_empty() {
                    fallback_content
                } else {
                    content
                },
                examples,
                relations: common::extract_links(item, "reference"),
                ..Default::default()
            });
        }
    }
    (!items.is_empty()).then(|| DictionarySection {
        kind: kind.to_string(),
        label: Some(label.to_string()),
        items,
    })
}

fn crown_example(
    source: &HtmlElement,
    translation: Option<&str>,
) -> Option<DictionaryExample> {
    let mut source_html = String::new();
    render_crown_inline(&source.children, &mut source_html);
    let source_html = common::normalize_visible_text(&source_html);
    if source_html.is_empty() {
        return None;
    }
    Some(DictionaryExample {
        source: common::html_text("ja", source_html),
        translation: translation
            .map(common::normalize_visible_text)
            .filter(|value| !value.is_empty())
            .map(|value| common::text("zh-CN", value)),
        ..Default::default()
    })
}

fn render_crown_inline(nodes: &[HtmlNode], output: &mut String) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => output.push_str(&common::escape_html(text)),
            HtmlNode::Element(element) if element.has_class("ruby_box") => {
                let base = class_text(element, "mj_rb").unwrap_or_default();
                let reading = class_text(element, "mj_rt").unwrap_or_default();
                if base.is_empty() || reading.is_empty() {
                    render_crown_inline(&element.children, output);
                } else {
                    output.push_str("<ruby>");
                    output.push_str(&common::escape_html(&base));
                    output.push_str("<rt>");
                    output.push_str(&common::escape_html(&reading));
                    output.push_str("</rt></ruby>");
                }
            }
            HtmlNode::Element(element) if element.has_class("pinyin_box") => {}
            HtmlNode::Element(element) => render_crown_inline(&element.children, output),
        }
    }
}

fn foreign_origin(raw_headword: &str) -> Option<String> {
    let start = raw_headword.find('〖')?;
    let end = raw_headword[start + '〖'.len_utf8()..].find('〗')? + start + '〖'.len_utf8();
    let value = raw_headword[start + '〖'.len_utf8()..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}
