use regex::Regex;
use std::sync::LazyLock;

/// 阅读器 Markdown 到 `analysisText` 的协议版本。
///
/// 修改清理、块间距或正文保留规则时必须升版，并同步共享 fixture。
pub const ANALYSIS_TEXT_PROTOCOL_VERSION: &str = "kotoclip.reader-analysis-text.v1";

static FRONTMATTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\u{feff}?---\r?\n.*?\r?\n---(?:\r?\n|$)").unwrap());
static RAW_HTML_FENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(```|~~~)\s*(?:\{=html\}|html|svg)\s*$").unwrap());
static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})[ \t]+(.+?)\s*$").unwrap());
static EMPTY_ANCHOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[\]\{#[^}]+\}\s*$").unwrap());
static PANDOC_DIV: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*:::(?:\s+\S.*)?\s*$").unwrap());
static STANDALONE_HTML_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^</?(?:div|svg)(?:\s[^>]*)?>$").unwrap());
static HORIZONTAL_RULE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[-*_]\s*){3,}$").unwrap());
static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"!\[([^\]]*)\]\((?:<([^>]+)>|([^\s)]+))(?:\s+["']([^"']*)["'])?\)(?:\{[^}]*\})?"#)
        .unwrap()
});
static LINKED_IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(!\[[^\]]*\]\([^)]+\)(?:\{[^}]*\})?)\]\([^)]+\)(?:\{[^}]*\})?").unwrap()
});
static MARKDOWN_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]\([^)]+\)").unwrap());
static EPUB_TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:x?html?|toc[-_#]|a_m\d+|b_m\d+)").unwrap());
static PANDOC_RUBY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"`<ruby>`\{=html\}(.+?)`<rt>`\{=html\}(.+?)`</rt>`\{=html\}`</ruby>`\{=html\}")
        .unwrap()
});
static WIKI_ALIAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[#[^\]|]+\|([^\]]+)\]\]").unwrap());
static WIKI_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[#([^\]]+)\]\]").unwrap());
static PANDOC_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\[\]]*)\]\{[^}\n]*\}").unwrap());
static INLINE_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\((?:<[^>]+>|[^)]+)\)(?:\{[^}]*\})?").unwrap());
static ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(?:[.#][^}\n]*|[^}\n]*=[^}\n]*)\}").unwrap());
static ENTITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)&(#x[\da-f]+|#\d+|[a-z]+);").unwrap());
static STRONG_STAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*(.+?)\*\*").unwrap());
static STRONG_UNDERSCORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"__(.+?)__").unwrap());
static STRIKETHROUGH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"~~(.+?)~~").unwrap());
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());
static BLOCKQUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*>\s?").unwrap());
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-+*]\s+").unwrap());
static ESCAPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\([\\`*{}\[\]()#+.!_-])").unwrap());

/// 将 EPUB 导入产出的规范 Markdown 编译为与 Web 阅读器一致的分析正文。
pub fn compile_analysis_text(source: &str) -> String {
    let body = extract_body(source)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut blocks = Vec::new();
    let mut paragraph_lines = Vec::new();
    let mut raw_fence: Option<String> = None;
    let mut skipping_navigation = false;

    for raw_line in body.split('\n') {
        let trimmed = raw_line.trim();
        if let Some(fence) = raw_fence.as_deref() {
            if trimmed.starts_with(fence) {
                raw_fence = None;
            }
            continue;
        }
        if let Some(captures) = RAW_HTML_FENCE.captures(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            raw_fence = Some(captures[1].to_string());
            continue;
        }
        if EMPTY_ANCHOR.is_match(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            continue;
        }
        if PANDOC_DIV.is_match(trimmed) || STANDALONE_HTML_BLOCK.is_match(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            continue;
        }
        if (trimmed.starts_with('\\') && trimmed[1..].trim().is_empty())
            || HORIZONTAL_RULE.is_match(trimmed)
        {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            continue;
        }
        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            continue;
        }
        if is_epub_notice(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            continue;
        }
        if let Some(captures) = HEADING.captures(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            let title = normalize_spaced_title(&clean_inline(&captures[2]));
            if title.is_empty() {
                continue;
            }
            if is_toc_title(&title) {
                skipping_navigation = true;
                continue;
            }
            skipping_navigation = false;
            if blocks.last() != Some(&title) {
                blocks.push(title);
            }
            continue;
        }
        if looks_like_navigation_line(trimmed) {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            skipping_navigation = true;
            continue;
        }

        let unwrapped = LINKED_IMAGE.replace_all(raw_line, "$1");
        let images: Vec<_> = IMAGE.captures_iter(&unwrapped).collect();
        if !images.is_empty() {
            skipping_navigation = false;
            let mut cursor = 0;
            for image in images {
                let whole = image.get(0).unwrap();
                let prefix = clean_inline(&unwrapped[cursor..whole.start()]);
                if !prefix.is_empty() {
                    paragraph_lines.push(prefix);
                }
                flush_paragraph(&mut paragraph_lines, &mut blocks);
                cursor = whole.end();
            }
            let suffix = clean_inline(&unwrapped[cursor..]);
            if !suffix.is_empty() {
                paragraph_lines.push(suffix);
            }
            continue;
        }
        if skipping_navigation {
            continue;
        }
        let cleaned = clean_inline(raw_line);
        if !cleaned.is_empty() {
            paragraph_lines.push(cleaned);
        }
    }
    flush_paragraph(&mut paragraph_lines, &mut blocks);
    blocks.join("\n\n")
}

fn extract_body(source: &str) -> &str {
    if let Some(found) = FRONTMATTER.find(source) {
        source[found.end()..].trim_start_matches(['\r', '\n'])
    } else {
        source.strip_prefix('\u{feff}').unwrap_or(source)
    }
}

fn flush_paragraph(lines: &mut Vec<String>, blocks: &mut Vec<String>) {
    if !lines.is_empty() {
        let text = lines.join("\n");
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            blocks.push(trimmed.to_string());
        }
        lines.clear();
    }
}

fn normalize_spaced_title(value: &str) -> String {
    let characters: Vec<char> = value.trim().chars().collect();
    let visible: Vec<_> = characters
        .iter()
        .enumerate()
        .filter(|(_, character)| !character.is_whitespace())
        .collect();
    if visible.len() < 3 {
        return value.trim().to_string();
    }
    let spaced_boundaries = visible
        .windows(2)
        .filter(|pair| {
            let previous = pair[0].0;
            let current = pair[1].0;
            current > previous + 1
                && characters[previous + 1..current]
                    .iter()
                    .all(|character| character.is_whitespace())
        })
        .count();
    if spaced_boundaries >= 2 && spaced_boundaries * 2 >= visible.len() - 1 {
        visible
            .into_iter()
            .map(|(_, character)| character)
            .collect()
    } else {
        value.trim().to_string()
    }
}

fn is_toc_title(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "目次" | "もくじ" | "contents" | "table of contents"
    )
}

fn is_epub_notice(value: &str) -> bool {
    value.contains("この本は縦書きでレイアウトされています")
        || value.contains("ご覧になる機種により、表示の差が認められることがあります")
        || value.contains("ご覧になる機種により表示の差が認められることがあります")
}

fn looks_like_navigation_line(line: &str) -> bool {
    let normalized = normalize_spaced_title(line);
    let markdown_links = MARKDOWN_LINK.find_iter(line).count();
    let epub_targets = EPUB_TARGET.find_iter(line).count();
    is_toc_title(&normalized)
        || (normalized.to_lowercase().starts_with("contents") && markdown_links > 0)
        || (markdown_links >= 2 && epub_targets >= 2)
        || line.trim_start().starts_with("- [[#")
        || line.trim_start().starts_with("* [[#")
}

fn clean_inline(source: &str) -> String {
    let mut value = PANDOC_RUBY.replace_all(source, "$1《$2》").into_owned();
    value = WIKI_ALIAS.replace_all(&value, "$1").into_owned();
    value = WIKI_LINK.replace_all(&value, "$1").into_owned();
    loop {
        let cleaned = PANDOC_SPAN.replace_all(&value, "$1").into_owned();
        if cleaned == value {
            break;
        }
        value = cleaned;
    }
    value = INLINE_LINK.replace_all(&value, "$1").into_owned();
    value = ATTRIBUTE.replace_all(&value, "").into_owned();
    value = strip_html_tags(&value);
    value = decode_html_entities(&value);
    value = STRONG_STAR.replace_all(&value, "$1").into_owned();
    value = STRONG_UNDERSCORE.replace_all(&value, "$1").into_owned();
    value = STRIKETHROUGH.replace_all(&value, "$1").into_owned();
    value = INLINE_CODE.replace_all(&value, "$1").into_owned();
    value = BLOCKQUOTE.replace(&value, "").into_owned();
    value = LIST_ITEM.replace(&value, "• ").into_owned();
    value = ESCAPE.replace_all(&value, "$1").into_owned();
    value.trim().to_string()
}

fn strip_html_tags(value: &str) -> String {
    let mut result = String::new();
    let mut inside_tag = false;
    for character in value.chars() {
        if character == '<' {
            inside_tag = true;
        } else if inside_tag && character == '>' {
            inside_tag = false;
        } else if !inside_tag {
            result.push(character);
        }
    }
    result
}

fn decode_html_entities(value: &str) -> String {
    ENTITY
        .replace_all(value, |captures: &regex::Captures<'_>| {
            let name = &captures[1];
            let decoded = if let Some(hexadecimal) =
                name.strip_prefix("#x").or_else(|| name.strip_prefix("#X"))
            {
                u32::from_str_radix(hexadecimal, 16)
                    .ok()
                    .and_then(char::from_u32)
            } else if let Some(decimal) = name.strip_prefix('#') {
                decimal.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                match name.to_ascii_lowercase().as_str() {
                    "amp" => Some('&'),
                    "apos" => Some('\''),
                    "gt" => Some('>'),
                    "lt" => Some('<'),
                    "nbsp" => Some(' '),
                    "quot" => Some('"'),
                    _ => None,
                }
            };
            decoded
                .map(|character| character.to_string())
                .unwrap_or_else(|| captures[0].to_string())
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        name: String,
        source: String,
        analysis_text: String,
    }

    #[test]
    fn shared_reader_fixtures_match_analysis_text() {
        let fixtures: Vec<Fixture> = serde_json::from_str(include_str!(
            "../../../src/reader/fixtures/markdown_analysis_cases.json"
        ))
        .unwrap();
        for fixture in fixtures {
            assert_eq!(
                compile_analysis_text(&fixture.source),
                fixture.analysis_text,
                "{}",
                fixture.name
            );
        }
    }
}
