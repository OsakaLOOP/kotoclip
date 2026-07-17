use crate::models::{DictionaryContentBlock, DictionaryLink};
use ammonia::Builder;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

const MAX_DEFINITION_BYTES: usize = 512 * 1024;
static SHOGAKUKAN_READING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<span\s+class="pinyin_h">([^<]+)</span>"#).unwrap());
static ENTRY_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a\s+[^>]*href=(?:['\"])entry://([^'\"]+)(?:['\"])[^>]*>(.*?)</a>"#).unwrap()
});
static HTML_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static DAIJIRIN_STRUCTURAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)〈(?:親項目|子項目|句項目)〉.*?(</p>|$)").unwrap());
static DAIJIRIN_ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)(?:⇔|→|⇒|☞)?\s*<a\s+[^>]*href=(?:['\"])entry://[^'\"]+(?:['\"])[^>]*>.*?</a>"#)
        .unwrap()
});
static DAIJIRIN_FIGURE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<img\s+[^>]*/?>").unwrap());
static DAIJIRIN_SEPARATOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\s|&nbsp;|；|;)+(</?(?:br|p)[^>]*>)").unwrap());
static DAIJIRIN_TRAILING_SEPARATOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:・|；|;)+\s*([。]|</|<br)").unwrap());
static LINK_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<link\s+[^>]*>"#).unwrap());
static SHOGAKUKAN_HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<h3>.*?</h3>"#).unwrap());
static SHOGAKUKAN_REFERENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:参见：|參見：)?\s*<a\s+[^>]*href=(?:['\"])entry://[^'\"]+(?:['\"])[^>]*>.*?</a>"#)
        .unwrap()
});
static CROWN_ANCHOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<a\s+name=[^>]*>\s*</a>"#).unwrap());
static CROWN_HEADWORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<div\s+class="midashi">.*?</div>"#).unwrap());

#[derive(Clone)]
pub struct DictionaryPresentation {
    pub definition_html: String,
    pub style_profile: String,
    pub content_blocks: Vec<DictionaryContentBlock>,
    pub links: Vec<DictionaryLink>,
    pub reading: Option<String>,
}

pub fn present(dict_name: &str, headword: &str, definition: &str) -> DictionaryPresentation {
    if dict_name.contains("大辞林") {
        present_daijirin(headword, definition)
    } else if dict_name.contains("小学館") || dict_name.contains("小学馆") {
        present_shogakukan(headword, definition)
    } else if dict_name.contains("CROWN") || dict_name.contains("Crown") {
        present_crown(headword, definition)
    } else {
        present_generic(definition)
    }
}

fn present_daijirin(headword: &str, definition: &str) -> DictionaryPresentation {
    let links = extract_daijirin_links(definition);
    let managed = clean_daijirin_markup(definition, headword, links.len());
    let mut presentation = finish("daijirin", managed, links);
    presentation.reading = headword
        .find(|character| matches!(character, '【' | '〖' | '（'))
        .map(|end| headword[..end].trim().to_string())
        .filter(|reading| is_kana(reading));
    presentation
}

fn present_shogakukan(_headword: &str, definition: &str) -> DictionaryPresentation {
    let links = extract_shogakukan_links(definition);
    let managed = clean_shogakukan_markup(definition);
    let mut presentation = finish("shogakukan", managed, links);

    presentation.reading = SHOGAKUKAN_READING_RE.captures(definition)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|reading| is_kana(reading));
    presentation
}

fn present_crown(headword: &str, definition: &str) -> DictionaryPresentation {
    let links = extract_crown_links(definition);
    let managed = clean_crown_markup(definition);
    let mut presentation = finish("crown", managed, links);
    presentation.reading = headword
        .find(|character| matches!(character, '【' | '〖' | '（'))
        .map(|end| headword[..end].trim().to_string())
        .filter(|reading| is_kana(reading));
    presentation
}

fn present_generic(definition: &str) -> DictionaryPresentation {
    finish("generic", definition.to_string(), Vec::new())
}

fn finish(profile: &str, source: String, links: Vec<DictionaryLink>) -> DictionaryPresentation {
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
        "a",
        "hy",
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
        "code",
        "mark",
        "vert",
        "v",
        "nh",
        "kh",
        "ku",
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
        .add_tag_attributes("p", &["class", "level", "no", "type", "delimiter", "data-orgtag"])
        .add_tag_attributes("section", &["class"])
        .add_tag_attributes("jae", &["class"])
        .add_tag_attributes("ja_cn", &["class"])
        .clean(&source)
        .to_string();
    let definition_html = truncate(clean);
    let content_blocks = (!definition_html.trim().is_empty())
        .then(|| DictionaryContentBlock {
            kind: "rich_text".to_string(),
            label: None,
            html: definition_html.clone(),
        })
        .into_iter()
        .collect();
    DictionaryPresentation {
        definition_html,
        style_profile: profile.to_string(),
        content_blocks,
        links,
        reading: None,
    }
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

fn extract_daijirin_links(definition: &str) -> Vec<DictionaryLink> {
    if let Some(target) = definition.strip_prefix("@@@LINK=") {
        return vec![DictionaryLink {
            target: target.trim().to_string(),
            label: target.trim().to_string(),
            relation: "redirect".to_string(),
        }];
    }
    let navigation = is_navigation_definition(definition);
    let mut seen = HashSet::new();
    ENTRY_LINK_RE
        .captures_iter(definition)
        .filter_map(|captures| {
            let target = captures.get(1)?.as_str().trim().to_string();
            if target.is_empty() || !seen.insert(target.clone()) {
                return None;
            }
            if navigation
                && !target.contains('【')
                && !target.contains('〖')
                && !target.contains('（')
            {
                return None;
            }
            let label = if navigation {
                target.clone()
            } else {
                HTML_TAG_RE
                    .replace_all(captures.get(2)?.as_str(), "")
                    .trim()
                    .to_string()
            };
            let before = &definition[..captures.get(0)?.start()];
            Some(DictionaryLink {
                target,
                label,
                relation: if navigation {
                    "candidate"
                } else {
                    classify_relation(before)
                }
                .to_string(),
            })
        })
        .collect()
}

fn classify_relation(before: &str) -> &'static str {
    let boundary = [
        before.rfind("<br"),
        before.rfind("</div>"),
        before.rfind("</p>"),
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(0);
    let context = &before[boundary..];
    if context.contains("親項目") {
        "parent"
    } else if context.contains("子項目") {
        "child"
    } else if context.contains("句項目") {
        "phrase"
    } else if context.contains("対義") || context.contains("反義") || context.contains('⇔') {
        "antonym"
    } else if context.contains("類語") || context.contains("同義") || context.contains("同意")
    {
        "synonym"
    } else if context.contains('→') || context.contains('⇒') || context.contains("参照") {
        "reference"
    } else {
        "related"
    }
}

fn clean_daijirin_markup(definition: &str, headword: &str, link_count: usize) -> String {
    if link_count >= 2 && is_kana(headword) && is_navigation_definition(definition) {
        return String::new();
    }
    let without_structural = DAIJIRIN_STRUCTURAL_RE.replace_all(definition, "$1");
    let without_anchors = DAIJIRIN_ANCHOR_RE.replace_all(&without_structural, "");
    let with_figures = DAIJIRIN_FIGURE_RE.replace_all(&without_anchors, |captures: &regex::Captures<'_>| {
        let tag = captures
            .get(0)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let label = if tag.contains("gaiji") {
            "外字"
        } else if tag.contains("glyph") {
            "图示"
        } else {
            "图版"
        };
        format!("<span class=\"media-omitted\">〔{label}〕</span>")
    });
    let normalized = DAIJIRIN_SEPARATOR_RE.replace_all(&with_figures, "$1");
    DAIJIRIN_TRAILING_SEPARATOR_RE
        .replace_all(&normalized, "$1")
        .into_owned()
}

fn is_navigation_definition(definition: &str) -> bool {
    definition.contains('☞')
        && !definition.contains("class=\"bss\"")
        && !definition.contains("class='bss'")
}

fn is_kana(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| ('\u{3041}'..='\u{30ff}').contains(&character) || character == 'ー')
}

fn extract_shogakukan_links(definition: &str) -> Vec<DictionaryLink> {
    if let Some(target) = definition.strip_prefix("@@@LINK=") {
        return vec![DictionaryLink {
            target: target.trim().to_string(),
            label: target.trim().to_string(),
            relation: "redirect".to_string(),
        }];
    }
    let mut seen = HashSet::new();
    ENTRY_LINK_RE
        .captures_iter(definition)
        .filter_map(|captures| {
            let target = captures.get(1)?.as_str().trim().to_string();
            if target.is_empty() || !seen.insert(target.clone()) {
                return None;
            }
            let label = HTML_TAG_RE
                .replace_all(captures.get(2)?.as_str(), "")
                .trim()
                .to_string();
            let before = &definition[..captures.get(0)?.start()];
            let relation = if before.contains("参见") || before.contains("參見") {
                "reference"
            } else {
                "related"
            };
            Some(DictionaryLink {
                target,
                label,
                relation: relation.to_string(),
            })
        })
        .collect()
}

fn extract_crown_links(definition: &str) -> Vec<DictionaryLink> {
    if let Some(target) = definition.strip_prefix("@@@LINK=") {
        return vec![DictionaryLink {
            target: target.trim().to_string(),
            label: target.trim().to_string(),
            relation: "redirect".to_string(),
        }];
    }
    let mut seen = HashSet::new();
    ENTRY_LINK_RE
        .captures_iter(definition)
        .filter_map(|captures| {
            let target = captures.get(1)?.as_str().trim().to_string();
            if target.is_empty() || !seen.insert(target.clone()) {
                return None;
            }
            let label = HTML_TAG_RE
                .replace_all(captures.get(2)?.as_str(), "")
                .trim()
                .to_string();
            Some(DictionaryLink {
                target,
                label,
                relation: "related".to_string(),
            })
        })
        .collect()
}

fn clean_shogakukan_markup(definition: &str) -> String {
    let without_link = LINK_TAG_RE.replace_all(definition, "");
    let without_h3 = SHOGAKUKAN_HEADING_RE.replace_all(&without_link, "");
    let without_ref = SHOGAKUKAN_REFERENCE_RE.replace_all(&without_h3, "");

    without_ref.into_owned()
}

fn clean_crown_markup(definition: &str) -> String {
    let without_link = LINK_TAG_RE.replace_all(definition, "");
    let without_anchor = CROWN_ANCHOR_RE.replace_all(&without_link, "");
    let without_midashi = CROWN_HEADWORD_RE.replace_all(&without_anchor, "");

    without_midashi.into_owned()
}
