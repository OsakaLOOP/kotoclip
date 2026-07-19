use super::AdaptedOccurrence;
use crate::dictionary::html::{HtmlElement, HtmlNode};
use crate::models::{
    DictionaryAdapterDiagnostics, DictionaryContentBlock, DictionaryExample, DictionaryGlossGroup,
    DictionaryLink, DictionaryOccurrenceHeader, DictionarySection, DictionarySense, DictionaryTag,
    DictionaryText,
};
use ammonia::Builder;
use std::collections::HashSet;

const MAX_DEFINITION_BYTES: usize = 512 * 1024;

pub fn fallback(
    profile: &str,
    display_form: &str,
    reading: Option<&str>,
    definition: &str,
    entry_kind: &str,
) -> AdaptedOccurrence {
    let definition_html = sanitize_fallback(definition);
    let content_blocks = (!definition_html.trim().is_empty())
        .then(|| DictionaryContentBlock {
            kind: "rich_text".to_string(),
            label: None,
            html: definition_html.clone(),
        })
        .into_iter()
        .collect();
    AdaptedOccurrence {
        source_record_index: 0,
        occurrence_suffix: String::new(),
        entry_kind: entry_kind.to_string(),
        header: DictionaryOccurrenceHeader {
            display_form: display_form.to_string(),
            canonical_form: Some(display_form.to_string()),
            reading: reading.map(str::to_string),
            ..Default::default()
        },
        definition_html,
        style_profile: profile.to_string(),
        content_blocks,
        diagnostics: DictionaryAdapterDiagnostics {
            coverage: "fallback".to_string(),
            warnings: vec!["词典专用结构未完整解析，当前显示安全降级内容".to_string()],
            omitted: Vec::new(),
        },
        ..Default::default()
    }
}

pub fn finish(
    mut occurrence: AdaptedOccurrence,
    profile: &str,
    fallback_source: &str,
) -> AdaptedOccurrence {
    occurrence.style_profile = profile.to_string();
    if occurrence.entry_kind.is_empty() {
        occurrence.entry_kind = "unknown".to_string();
    }
    if occurrence.header.display_form.is_empty() {
        occurrence.header.display_form = "未识别词头".to_string();
    }
    let structured = render_structured_html(&occurrence.senses, &occurrence.sections);
    occurrence.definition_html = if structured.trim().is_empty() {
        sanitize_fallback(fallback_source)
    } else {
        structured
    };
    occurrence.content_blocks = (!occurrence.definition_html.trim().is_empty())
        .then(|| DictionaryContentBlock {
            kind: if occurrence.senses.is_empty() && occurrence.sections.is_empty() {
                "rich_text"
            } else {
                "structured_preview"
            }
            .to_string(),
            label: None,
            html: occurrence.definition_html.clone(),
        })
        .into_iter()
        .collect();
    if occurrence.diagnostics.coverage.is_empty() {
        occurrence.diagnostics.coverage = if occurrence.senses.is_empty() {
            "partial"
        } else {
            "structured"
        }
        .to_string();
    }
    occurrence
}

pub fn text(lang: &str, value: impl AsRef<str>) -> DictionaryText {
    DictionaryText {
        lang: lang.to_string(),
        qualifier: None,
        html: escape_html(&normalize_visible_text(value.as_ref())),
    }
}

pub fn qualified_text(
    lang: &str,
    qualifier: impl AsRef<str>,
    value: impl AsRef<str>,
) -> DictionaryText {
    let qualifier = normalize_visible_text(qualifier.as_ref());
    DictionaryText {
        lang: lang.to_string(),
        qualifier: (!qualifier.is_empty()).then_some(qualifier),
        html: escape_html(&normalize_visible_text(value.as_ref())),
    }
}

pub fn html_text(lang: &str, value: impl AsRef<str>) -> DictionaryText {
    DictionaryText {
        lang: lang.to_string(),
        qualifier: None,
        html: sanitize_fallback(value.as_ref()).trim().to_string(),
    }
}

pub fn tag(kind: &str, label: impl AsRef<str>) -> DictionaryTag {
    DictionaryTag {
        kind: kind.to_string(),
        label: normalize_visible_text(label.as_ref()),
    }
}

pub fn example(source: &str, translation: Option<&str>) -> Option<DictionaryExample> {
    let source = normalize_visible_text(source);
    if source.is_empty() {
        return None;
    }
    Some(DictionaryExample {
        source: text("ja", source),
        translation: translation
            .map(normalize_visible_text)
            .filter(|value| !value.is_empty())
            .map(|value| text("zh-CN", value)),
        note: None,
    })
}

pub fn normalize_visible_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_space = false;
    for character in value.chars() {
        let character = match character {
            '\u{00a0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200a}' | '\u{3000}' => ' ',
            '．' => '。',
            _ => character,
        };
        if character.is_whitespace() {
            if !previous_space && !output.is_empty() {
                output.push(' ');
            }
            previous_space = true;
        } else {
            output.push(character);
            previous_space = false;
        }
    }
    let mut output = output.trim().to_string();
    for repeated in ["。。", "。。", "；；", "，，", "、、", "⇒⇒"] {
        while output.contains(repeated) {
            output = output.replace(
                repeated,
                &repeated[..repeated.chars().next().unwrap().len_utf8()],
            );
        }
    }
    output = output
        .replace("。 。", "。")
        .replace("， ，", "，")
        .replace("【・】", "")
        .replace("┏", "")
        .replace("┓", "")
        .replace("┗", "")
        .replace("┛", "");
    output.trim().to_string()
}

pub fn normalize_reading(value: &str) -> String {
    normalize_visible_text(value)
        .replace('・', "")
        .replace('･', "")
}

pub fn extract_links(element: &HtmlElement, default_relation: &str) -> Vec<DictionaryLink> {
    let mut anchors = Vec::new();
    element.all_by_name("a", &mut anchors);
    let mut seen = HashSet::new();
    anchors
        .into_iter()
        .filter_map(|anchor| {
            let target = anchor.attr("href")?.strip_prefix("entry://")?.trim();
            if target.is_empty() || !seen.insert(target.to_string()) {
                return None;
            }
            let label = normalize_visible_text(&anchor.text());
            Some(DictionaryLink {
                target: target.to_string(),
                label: if label.is_empty() {
                    target.to_string()
                } else {
                    label
                },
                relation: default_relation.to_string(),
            })
        })
        .collect()
}

pub fn direct_child_elements(element: &HtmlElement) -> impl Iterator<Item = &HtmlElement> {
    element.children.iter().filter_map(|child| match child {
        HtmlNode::Element(element) => Some(element),
        HtmlNode::Text(_) => None,
    })
}

pub fn sanitize_fallback(source: &str) -> String {
    let allowed: HashSet<&str> = [
        "p",
        "div",
        "span",
        "br",
        "ruby",
        "rt",
        "rp",
        "b",
        "strong",
        "i",
        "em",
        "ul",
        "ol",
        "li",
        "dl",
        "dt",
        "dd",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "sup",
        "sub",
        "small",
        "blockquote",
        "section",
        "jae",
        "ja_cn",
    ]
    .into_iter()
    .collect();
    let clean = Builder::default()
        .tags(allowed)
        .add_tag_attributes("span", &["class", "type"])
        .add_tag_attributes("div", &["class", "id", "type", "delimiter", "data-orgtag"])
        .add_tag_attributes(
            "p",
            &["class", "level", "no", "type", "delimiter", "data-orgtag"],
        )
        .add_tag_attributes("section", &["class"])
        .clean(source)
        .to_string();
    truncate(clean)
}

fn render_structured_html(senses: &[DictionarySense], sections: &[DictionarySection]) -> String {
    let mut output = String::new();
    if !senses.is_empty() {
        render_sense_list(senses, 0, &mut output);
    }
    for section in sections {
        output.push_str("<section class=\"dictionary-section\" data-kind=\"");
        output.push_str(&escape_html(&section.kind));
        output.push_str("\">");
        if let Some(label) = &section.label {
            output.push_str("<h4 class=\"dictionary-section__title\">");
            output.push_str(&escape_html(label));
            output.push_str("</h4>");
        }
        for item in &section.items {
            output.push_str("<article class=\"dictionary-section__item\">");
            if item.label.is_some() || item.reading.is_some() || !item.tags.is_empty() {
                output.push_str("<header class=\"dictionary-section__header\">");
                if let Some(label_html) = &item.label_html {
                    output.push_str("<strong lang=\"ja\">");
                    output.push_str(label_html);
                    output.push_str("</strong>");
                } else if let Some(label) = &item.label {
                    output.push_str("<strong lang=\"ja\">");
                    output.push_str(&escape_html(label));
                    output.push_str("</strong>");
                }
                if let Some(reading) = &item.reading {
                    output.push_str("<span class=\"dictionary-section__reading\" lang=\"ja\">【");
                    output.push_str(&escape_html(reading));
                    output.push_str("】</span>");
                }
                render_tags(&item.tags, &mut output);
                output.push_str("</header>");
            }
            for content in &item.content {
                output.push_str("<div class=\"dictionary-section__content\"");
                if !content.lang.is_empty() {
                    output.push_str(" lang=\"");
                    output.push_str(&escape_html(&content.lang));
                    output.push('"');
                }
                output.push('>');
                output.push_str(&content.html);
                output.push_str("</div>");
            }
            if !item.senses.is_empty() {
                render_sense_list(&item.senses, 0, &mut output);
            }
            render_examples(&item.examples, &mut output);
            render_relations(&item.relations, &mut output);
            output.push_str("</article>");
        }
        output.push_str("</section>");
    }
    truncate(output)
}

fn render_sense_list(senses: &[DictionarySense], depth: usize, output: &mut String) {
    output.push_str("<ol class=\"sense-tree sense-tree--depth-");
    output.push_str(&depth.min(2).to_string());
    output.push_str("\">");
    for sense in senses {
        render_sense(sense, depth, output);
    }
    output.push_str("</ol>");
}

fn render_sense(sense: &DictionarySense, depth: usize, output: &mut String) {
    output.push_str("<li class=\"sense-node\"><div class=\"sense-main");
    if sense.marker.is_none() {
        output.push_str(" sense-main--unmarked");
    }
    output.push_str("\">");
    if let Some(marker) = &sense.marker {
        output.push_str("<span class=\"sense-marker\">");
        output.push_str(&escape_html(marker));
        output.push_str("</span>");
    }
    output.push_str("<div class=\"sense-content\">");
    if !sense.gloss_groups.is_empty() {
        output.push_str("<div class=\"sense-gloss-groups\">");
        for group in &sense.gloss_groups {
            render_gloss_group(group, output);
        }
        output.push_str("</div>");
    } else if sense.heading.is_some() || !sense.tags.is_empty() || !sense.glosses.is_empty() {
        output.push_str("<div class=\"sense-gloss-group\">");
        if let Some(heading) = &sense.heading {
            output.push_str("<span class=\"sense-heading\" lang=\"ja\">");
            output.push_str(&escape_html(heading));
            output.push_str("</span>");
        }
        render_tags(&sense.tags, output);
        output.push_str("<span class=\"sense-gloss-clauses\">");
        let mut previous_qualifier: Option<&str> = None;
        for (index, gloss) in sense.glosses.iter().enumerate() {
            if index > 0 {
                output.push_str("<span class=\"sense-gloss-separator\">，</span>");
            }
            output.push_str("<span class=\"sense-gloss-clause\"");
            if !gloss.lang.is_empty() {
                output.push_str(" lang=\"");
                output.push_str(&escape_html(&gloss.lang));
                output.push('"');
            }
            output.push('>');
            if let Some(qualifier) = gloss
                .qualifier
                .as_deref()
                .filter(|qualifier| Some(*qualifier) != previous_qualifier)
            {
                output.push_str("<span class=\"sense-gloss__qualifier\" lang=\"ja\">");
                output.push_str(&escape_html(qualifier));
                output.push_str("</span>");
            }
            output.push_str(&gloss.html);
            output.push_str("</span>");
            previous_qualifier = gloss.qualifier.as_deref();
        }
        output.push_str("</span></div>");
    }
    for definition in &sense.definitions {
        output.push_str("<div class=\"sense-definition\"");
        if !definition.lang.is_empty() {
            output.push_str(" lang=\"");
            output.push_str(&escape_html(&definition.lang));
            output.push('"');
        }
        output.push('>');
        output.push_str(&definition.html);
        output.push_str("</div>");
    }
    render_examples(&sense.examples, output);
    for note in &sense.notes {
        output.push_str("<div class=\"sense-note\"");
        if !note.lang.is_empty() {
            output.push_str(" lang=\"");
            output.push_str(&escape_html(&note.lang));
            output.push('"');
        }
        output.push('>');
        output.push_str(&note.html);
        output.push_str("</div>");
    }
    render_relations(&sense.relations, output);
    output.push_str("</div></div>");
    if !sense.children.is_empty() {
        render_sense_list(&sense.children, depth + 1, output);
    }
    output.push_str("</li>");
}

fn render_gloss_group(group: &DictionaryGlossGroup, output: &mut String) {
    output.push_str("<div class=\"sense-gloss-group\">");
    if let Some(heading) = &group.heading {
        output.push_str("<span class=\"sense-heading\" lang=\"ja\">");
        output.push_str(&escape_html(heading));
        output.push_str("</span>");
    }
    output.push_str("<span class=\"sense-gloss-clauses\">");
    for clause in &group.clauses {
        if let Some(separator) = &clause.separator {
            output.push_str("<span class=\"sense-gloss-separator\">");
            output.push_str(&escape_html(separator));
            output.push_str("</span>");
        }
        output.push_str("<span class=\"sense-gloss-clause\"");
        if !clause.text.lang.is_empty() {
            output.push_str(" lang=\"");
            output.push_str(&escape_html(&clause.text.lang));
            output.push('"');
        }
        output.push('>');
        if let Some(qualifier) = &clause.qualifier {
            output.push_str("<span class=\"sense-gloss__qualifier\" lang=\"ja\">");
            output.push_str(&escape_html(qualifier));
            output.push_str("</span>");
        }
        render_tags(&clause.leading_tags, output);
        output.push_str(&clause.text.html);
        render_tags(&clause.trailing_tags, output);
        output.push_str("</span>");
    }
    output.push_str("</span></div>");
}

fn render_tags(tags: &[crate::models::DictionaryTag], output: &mut String) {
    for tag in tags {
        output.push_str("<span class=\"dictionary-tag\" data-kind=\"");
        output.push_str(&escape_html(&tag.kind));
        output.push_str("\">");
        output.push_str(&escape_html(&tag.label));
        output.push_str("</span>");
    }
}

fn render_example(example: &DictionaryExample, output: &mut String) {
    output.push_str("<blockquote class=\"example-pair\">");
    output.push_str("<div class=\"example-source\"");
    if !example.source.lang.is_empty() {
        output.push_str(" lang=\"");
        output.push_str(&escape_html(&example.source.lang));
        output.push('"');
    }
    output.push('>');
    output.push_str(&example.source.html);
    output.push_str("</div>");
    if let Some(translation) = &example.translation {
        output.push_str("<div class=\"example-translation\"");
        if !translation.lang.is_empty() {
            output.push_str(" lang=\"");
            output.push_str(&escape_html(&translation.lang));
            output.push('"');
        }
        output.push('>');
        output.push_str(&translation.html);
        output.push_str("</div>");
    }
    if let Some(note) = &example.note {
        output.push_str("<div class=\"example-note\"");
        if !note.lang.is_empty() {
            output.push_str(" lang=\"");
            output.push_str(&escape_html(&note.lang));
            output.push('"');
        }
        output.push('>');
        output.push_str(&note.html);
        output.push_str("</div>");
    }
    output.push_str("</blockquote>");
}

fn render_examples(examples: &[DictionaryExample], output: &mut String) {
    if examples.is_empty() {
        return;
    }
    output.push_str("<section class=\"example-browser preview-example-browser");
    output.push_str("\" data-example-browser>");
    if examples.len() > 2 {
        output.push_str("<div class=\"example-browser__status\" aria-live=\"polite\"><span data-example-counter></span><span data-example-total hidden></span></div>");
    }
    output.push_str("<div class=\"example-browser__viewport\"><div class=\"example-browser__page\">");
    for (index, example) in examples.iter().enumerate() {
        output.push_str("<div data-example-index=\"");
        output.push_str(&index.to_string());
        output.push_str("\">");
        render_example(example, output);
        output.push_str("</div>");
    }
    output.push_str("</div>");
    if examples.len() > 2 {
        output.push_str("<button type=\"button\" class=\"example-browser__nav example-browser__nav--previous\" data-example-previous aria-label=\"上一页例句\">‹</button><button type=\"button\" class=\"example-browser__nav example-browser__nav--next\" data-example-next aria-label=\"下一页例句\">›</button>");
    }
    output.push_str("</div>");
    if examples.len() > 2 {
        output.push_str("<footer class=\"example-browser__controls\"><button type=\"button\" class=\"example-browser__toggle\" data-example-toggle aria-expanded=\"false\">展开</button></footer>");
    }
    output.push_str("</section>");
}

fn render_relations(relations: &[DictionaryLink], output: &mut String) {
    if relations.is_empty() {
        return;
    }
    output.push_str("<div class=\"sense-relations\">");
    for relation in relations {
        output.push_str("<span class=\"sense-relation-static\" data-relation=\"");
        output.push_str(&escape_html(&relation.relation));
        output.push_str("\">");
        output.push_str(&escape_html(if relation.label.is_empty() {
            &relation.target
        } else {
            &relation.label
        }));
        output.push_str("</span>");
    }
    output.push_str("</div>");
}

pub fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn truncate(clean: String) -> String {
    if clean.len() <= MAX_DEFINITION_BYTES {
        return clean;
    }
    let mut limit = MAX_DEFINITION_BYTES;
    while limit > 0 && !clean.is_char_boundary(limit) {
        limit -= 1;
    }
    format!("{}… [内容已截断]", &clean[..limit])
}
