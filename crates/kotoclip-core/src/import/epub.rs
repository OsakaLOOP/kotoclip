use regex::Regex;
use roxmltree::{Document, Node};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportedEpub {
    pub source_name: String,
    pub title: String,
    pub author: String,
    pub markdown: String,
    pub chapter_titles: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum EpubImportError {
    #[error("无法打开 EPUB：{0}")]
    Io(#[from] std::io::Error),
    #[error("EPUB 压缩包无效：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("EPUB 缺少 OPF 元数据文件")]
    MissingOpf,
    #[error("EPUB 缺少 manifest 或 spine")]
    MissingSpine,
    #[error("EPUB XML 无效：{0}")]
    Xml(#[from] roxmltree::Error),
    #[error("EPUB 文本不是 UTF-8：{0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Clone)]
struct Metadata {
    title: String,
    author: String,
    date: String,
    language: String,
}

#[derive(Debug, Clone)]
struct Chapter {
    title: String,
    image: Option<String>,
}

pub fn import_epub(path: impl AsRef<Path>) -> Result<ImportedEpub, EpubImportError> {
    let path = path.as_ref();
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名.epub")
        .to_string();
    let fallback_title = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名")
        .to_string();
    let mut archive = ZipArchive::new(File::open(path)?)?;
    let opf_path = locate_opf(&mut archive)?;
    let opf = read_zip_text(&mut archive, &opf_path)?;
    let opf_document = Document::parse(&opf)?;
    let metadata = read_metadata(&opf_document, &fallback_title);
    let (manifest, spine) = read_spine(&opf_document)?;
    let mut warnings = Vec::new();
    let mut input = build_frontmatter(&metadata);

    for idref in spine {
        let Some(href) = manifest.get(&idref) else {
            warnings.push(format!("spine 项 {idref} 未出现在 manifest 中"));
            continue;
        };
        let requested = resolve_zip_path(&opf_path, href);
        let Some(zip_path) = find_zip_entry(&mut archive, &requested, href) else {
            warnings.push(format!("未找到正文文件：{href}"));
            continue;
        };
        let basename = zip_basename(href);
        if basename.eq_ignore_ascii_case("titlepage.xhtml") {
            input.push_str(titlepage_markdown());
            continue;
        }
        let xhtml = match read_zip_text(&mut archive, &zip_path) {
            Ok(text) => text,
            Err(error) => {
                warnings.push(format!("读取正文 {href} 失败：{error}"));
                continue;
            }
        };
        let document = match Document::parse(strip_xml_declaration(&xhtml)) {
            Ok(document) => document,
            Err(error) => {
                warnings.push(format!("解析正文 {href} 失败：{error}"));
                continue;
            }
        };
        let Some(body) = document
            .descendants()
            .find(|node| node.has_tag_name("body"))
        else {
            warnings.push(format!("正文 {href} 缺少 body"));
            continue;
        };
        input.push_str("\n[]{#");
        input.push_str(&basename);
        input.push_str("}\n");
        input.push_str(&render_children(body, &basename, false));
    }

    let input = input
        .split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .replace('…', "...");
    let (markdown, chapter_titles) = transform_content(&input);
    Ok(ImportedEpub {
        source_name,
        title: metadata.title,
        author: metadata.author,
        markdown,
        chapter_titles,
        warnings,
    })
}

fn locate_opf<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, EpubImportError> {
    if let Ok(container) = read_zip_text(archive, "META-INF/container.xml") {
        if let Ok(document) = Document::parse(&container) {
            if let Some(path) = document
                .descendants()
                .find(|node| node.has_tag_name("rootfile"))
                .and_then(|node| node.attribute("full-path"))
            {
                return Ok(path.to_string());
            }
        }
    }
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.name().to_ascii_lowercase().ends_with(".opf") {
            return Ok(file.name().to_string());
        }
    }
    Err(EpubImportError::MissingOpf)
}

fn read_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, EpubImportError> {
    let mut file = archive.by_name(name)?;
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes)?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(..3);
    }
    Ok(String::from_utf8(bytes)?)
}

fn read_metadata(document: &Document<'_>, fallback_title: &str) -> Metadata {
    let text = |name: &str| {
        document
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == name)
            .and_then(|node| node.text())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    Metadata {
        title: text("title").unwrap_or_else(|| fallback_title.to_string()),
        author: text("creator").unwrap_or_else(|| "未知".to_string()),
        date: text("date").unwrap_or_else(|| "未知".to_string()),
        language: text("language").unwrap_or_else(|| "ja".to_string()),
    }
}

fn read_spine(
    document: &Document<'_>,
) -> Result<(HashMap<String, String>, Vec<String>), EpubImportError> {
    let manifest_node = document
        .descendants()
        .find(|node| node.has_tag_name("manifest"))
        .ok_or(EpubImportError::MissingSpine)?;
    let spine_node = document
        .descendants()
        .find(|node| node.has_tag_name("spine"))
        .ok_or(EpubImportError::MissingSpine)?;
    let manifest = manifest_node
        .children()
        .filter(|node| node.has_tag_name("item"))
        .filter_map(|node| Some((node.attribute("id")?, node.attribute("href")?)))
        .map(|(id, href)| (id.to_string(), href.to_string()))
        .collect();
    let spine = spine_node
        .children()
        .filter(|node| node.has_tag_name("itemref"))
        .filter_map(|node| node.attribute("idref"))
        .map(str::to_string)
        .collect();
    Ok((manifest, spine))
}

fn build_frontmatter(metadata: &Metadata) -> String {
    format!(
        "---\nauthor: {}\ncontributor: kotoclip EPUB importer\ndate: \"{}\"\nidentifier:\n- kotoclip-epub-import\nlanguage: {}\ntitle: {}\n---\n",
        metadata.author, metadata.date, metadata.language, metadata.title
    )
}

fn resolve_zip_path(opf_path: &str, href: &str) -> String {
    let mut segments = opf_path.split('/').collect::<Vec<_>>();
    segments.pop();
    let normalized_href = href.replace('\\', "/");
    for segment in normalized_href.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            value => segments.push(value),
        }
    }
    segments.join("/")
}

fn find_zip_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    requested: &str,
    href: &str,
) -> Option<String> {
    if archive.by_name(requested).is_ok() {
        return Some(requested.to_string());
    }
    let normalized_href = href.replace('\\', "/");
    for index in 0..archive.len() {
        let Ok(file) = archive.by_index(index) else {
            continue;
        };
        if file.name().replace('\\', "/").ends_with(&normalized_href) {
            return Some(file.name().to_string());
        }
    }
    None
}

fn zip_basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn strip_xml_declaration(input: &str) -> &str {
    let trimmed = input.trim_start_matches('\u{feff}').trim_start();
    if !trimmed.starts_with("<?xml") {
        return trimmed;
    }
    trimmed
        .find("?>")
        .map(|end| trimmed[end + 2..].trim_start())
        .unwrap_or(trimmed)
}

fn titlepage_markdown() -> &'static str {
    "\n![](./cover.jpeg)\n\n[]{#titlepage.xhtml}\n\n<div>\n\n```{=html}\n<svg xmlns=\"http://www.w3.org/2000/svg\">\n```\n`<image xlink:href=\"cover.jpeg\">`{=html}`</image>`{=html}\n```{=html}\n</svg>\n```\n\n</div>\n"
}

fn render_children(node: Node<'_, '_>, filename: &str, in_paragraph: bool) -> String {
    node.children()
        .map(|child| render_node(child, filename, in_paragraph))
        .collect()
}

fn render_node(node: Node<'_, '_>, filename: &str, in_paragraph: bool) -> String {
    if node.is_text() {
        let text = node.text().unwrap_or_default();
        return if in_paragraph || !trim_layout_whitespace(text).is_empty() {
            text.to_string()
        } else {
            String::new()
        };
    }
    if !node.is_element() {
        return String::new();
    }
    let tag = node.tag_name().name();
    let current_in_paragraph = in_paragraph || tag == "p";
    if tag == "ruby" {
        let reading = node
            .children()
            .find(|child| child.has_tag_name("rt"))
            .and_then(|child| child.text())
            .unwrap_or_default();
        let base = node
            .children()
            .filter(|child| !child.has_tag_name("rt") && !child.has_tag_name("rp"))
            .map(|child| render_node(child, filename, current_in_paragraph))
            .collect::<String>();
        return format!(
            "`<ruby>`{{=html}}{base}`<rt>`{{=html}}{reading}`</rt>`{{=html}}`</ruby>`{{=html}}"
        );
    }
    let content = render_children(node, filename, current_in_paragraph);
    match tag {
        "body" => content,
        "div" => node.attribute("class").map_or(content.clone(), |class| {
            format!("\n::: {class}\n{}\n:::\n", trim_layout_whitespace(&content))
        }),
        "p" => format!("\n{}\n", trim_layout_whitespace(&content)),
        "span" => node
            .attribute("class")
            .map_or(content.clone(), |class| format!("[{content}]{{.{class}}}")),
        "rt" | "rp" => String::new(),
        "a" => {
            let mut href = node.attribute("href").unwrap_or_default().to_string();
            if !href.starts_with('#') && (href.contains(".html") || href.contains(".xhtml")) {
                href.insert(0, '#');
            }
            let id = node
                .attribute("id")
                .map(|id| format!("#{filename}#{id}"))
                .unwrap_or_default();
            let classes = node
                .attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .map(|class| format!(".{class}"))
                .collect::<Vec<_>>()
                .join(" ");
            let attributes = match (id.is_empty(), classes.is_empty()) {
                (false, false) => format!("{id}\n{classes}"),
                (false, true) => id,
                (true, false) => classes,
                (true, true) => String::new(),
            };
            if attributes.is_empty() {
                format!("[{content}]({href})")
            } else {
                format!("[{content}]({href}){{{attributes}}}")
            }
        }
        "img" => {
            let image = zip_basename(node.attribute("src").unwrap_or_default());
            node.attribute("class").map_or_else(
                || format!("![](./{image})"),
                |class| format!("![](./{image}){{.{class}}}"),
            )
        }
        "br" => "\\\n".to_string(),
        _ => content,
    }
}

fn trim_layout_whitespace(input: &str) -> &str {
    input.trim_matches(['\r', '\n', '\t', ' '])
}

fn transform_content(input: &str) -> (String, Vec<String>) {
    let mut content = input.replace("\r\n", "\n").replace('\r', "\n");
    let chapter_image_pattern =
        Regex::new(r"\[(!\[\]\(([^)]+)\))\{[^}]*\}\]\(#([^)]+\.html#(a_m\d+))\)").unwrap();
    let image_by_anchor = chapter_image_pattern
        .captures_iter(&content)
        .map(|captures| (captures[4].to_string(), captures[2].to_string()))
        .collect::<HashMap<_, _>>();
    let toc_entry_pattern = Regex::new(r"\[(.+?)\]\([^)]+\)\{#([^}]+)\.html#(a_m\d+)").unwrap();
    let chapter_number_pattern = Regex::new(r"a_m0*(\d+)").unwrap();
    let mut chapters = BTreeMap::new();
    for captures in toc_entry_pattern.captures_iter(&content) {
        let Some(number) = chapter_number_pattern
            .captures(&captures[3])
            .and_then(|value| value[1].parse::<usize>().ok())
        else {
            continue;
        };
        chapters.insert(
            number,
            Chapter {
                title: captures[1].trim().to_string(),
                image: image_by_anchor.get(&captures[3]).cloned(),
            },
        );
    }

    let yaml_pattern = Regex::new(r"(?s)\A---\n(.*?)\n---").unwrap();
    if let Some(captures) = yaml_pattern.captures(&content) {
        let body = &captures[1];
        let field = |pattern: &str, fallback: &str| {
            Regex::new(pattern)
                .unwrap()
                .captures(body)
                .map(|value| value[1].trim().to_string())
                .unwrap_or_else(|| fallback.to_string())
        };
        let title = field(r"(?m)^title:\s*(.+)$", "未知");
        let author = field(r"(?m)^author:\s*(.+)$", "未知");
        let date = field(r#"(?m)^date:\s*"(\d{4}-\d{2}-\d{2})"#, "未知");
        let language = field(r"(?m)^language:\s*(.+)$", "ja");
        let replacement = format!(
            "---\ntitle: \"{title}\"\nauthor: \"{author}\"\ndate: \"{date}\"\nlanguage: \"{language}\"\n---"
        );
        content.replace_range(captures.get(0).unwrap().range(), &replacement);
    }

    content = Regex::new(r"(?s)```\{=html\}\n.*?\n```\n?")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();
    content = Regex::new(r"`<image[^>]*>`\{=html\}`</image>`\{=html\}\n?")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();
    content = Regex::new(r"(?m)^</?div>\s*$\n?")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();
    content = Regex::new(r"\[\]\{#.*?\}\n?")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();
    content =
        Regex::new(r"(?m)\[(!\[\]\([^)]+\))\{[^}]*\}\]\([^)]+\)\{#[^\n]*\n\.[^\n]*calibre[^\n]*\}")
            .unwrap()
            .replace_all(&content, "$1")
            .into_owned();
    content = Regex::new(r"(?m)\[(.+?)\]\([^)]+\)\{#[^\n]*\n\.[^\n]*calibre[^\n]*\}")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();
    content = Regex::new(r"(?m)^目次\s*$\n?")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();

    if let Some(cover_pos) = content.find("![](./cover.jpeg)") {
        let image_pattern = Regex::new(r"!\[\]\(\./00001\.jpeg\)").unwrap();
        if let Some(first_image) = image_pattern.find(&content[cover_pos..]) {
            let start = cover_pos + "![](./cover.jpeg)".len();
            let end = cover_pos + first_image.start();
            let cleaned = content[start..end]
                .lines()
                .filter(|line| line.trim().is_empty() || line.trim().starts_with('!'))
                .collect::<Vec<_>>()
                .join("\n");
            content.replace_range(start..end, &cleaned);
        }
    }

    content =
        Regex::new(r"`<ruby>`\{=html\}(.+?)`<rt>`\{=html\}(.+?)`</rt>`\{=html\}`</ruby>`\{=html\}")
            .unwrap()
            .replace_all(&content, "$1《$2》")
            .into_owned();
    content = content.replace("{=html}", "");
    for (pattern, replacement) in [
        (r"\[([^\]]+)\]\{\.em-sesame_f\}", "$1"),
        (r"\[([^\]]+)\]\{\.tcy\}", "$1"),
        (r"\[([^\]]+)\]\{\.font-size80per\}", "$1"),
        (r"\[([^\]]+)\]\{\.text-upright\}", "$1"),
        (r"\[([^\]]+)\]\{\.text-sideways\}", "$1"),
        (r"\{\.img-fit\}", ""),
        (r"\{\.gaiji\}", ""),
    ] {
        content = Regex::new(pattern)
            .unwrap()
            .replace_all(&content, replacement)
            .into_owned();
    }
    for class in ["center", "right", "middle-block"] {
        content = Regex::new(&format!(r"(?m)^::: {class}\s*$\n?"))
            .unwrap()
            .replace_all(&content, "")
            .into_owned();
    }
    content = Regex::new(r"(?m)^:::\s*$\n?")
        .unwrap()
        .replace_all(&content, "")
        .into_owned();
    content = Regex::new(r"(?m)^\\\s*$\n?")
        .unwrap()
        .replace_all(&content, "\n")
        .into_owned();

    for chapter in chapters.values() {
        let Some(image) = &chapter.image else {
            continue;
        };
        let image_reference = format!("![]({image})");
        if let Some(position) = content.find(&image_reference) {
            content.insert_str(position, &format!("\n## {}\n\n", chapter.title));
        }
    }
    content = content.replace("\n了\n", "\n了\n\n---\n");
    if let Some(position) = content.find("\nカミツキレイニー\n\nKamitsuki rainy") {
        content.insert_str(position, "\n## あとがき\n\n");
    }
    let colophon_position = content.find("\n小学館ｅＢｏｏｋｓ\n").or_else(|| {
        Regex::new(r"\n２０１５年")
            .unwrap()
            .find(&content)
            .map(|item| item.start())
    });
    if let Some(position) = colophon_position {
        content.insert_str(position, "\n## 奥付\n\n");
    }

    if let Some(cover_pos) = content.find("![](./cover.jpeg)") {
        let mut toc = String::from("## 目次\n\n");
        for chapter in chapters.values() {
            toc.push_str(&format!("- [[#{}]]\n", chapter.title));
        }
        if !chapters.is_empty() {
            toc.push('\n');
            content.insert_str(cover_pos, &toc);
        }
    }
    content = Regex::new(r"\n{4,}")
        .unwrap()
        .replace_all(&content, "\n\n\n")
        .into_owned();
    content = content
        .split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    content = format!("{}\n", content.trim_start_matches('\n'));
    let chapter_titles = chapters
        .values()
        .map(|chapter| chapter.title.clone())
        .collect();
    (content, chapter_titles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn renders_ruby_as_kotoclip_markdown() {
        let document = Document::parse(
            r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>彼は<ruby>本<rt>ほん</rt></ruby>を読む。</p></body></html>"#,
        )
        .unwrap();
        let body = document
            .descendants()
            .find(|node| node.has_tag_name("body"))
            .unwrap();
        let rendered = render_children(body, "chapter.xhtml", false);
        let (markdown, _) = transform_content(&format!(
            "---\nauthor: 著者\ndate: \"2026-07-19\"\nlanguage: ja\ntitle: 本\n---\n{rendered}"
        ));
        assert!(markdown.contains("彼は本《ほん》を読む。"));
    }

    #[test]
    fn resolves_manifest_paths_relative_to_opf() {
        assert_eq!(
            resolve_zip_path("OEBPS/content.opf", "text/chapter.xhtml"),
            "OEBPS/text/chapter.xhtml"
        );
        assert_eq!(
            resolve_zip_path("OPS/package/content.opf", "../text/chapter.xhtml"),
            "OPS/text/chapter.xhtml"
        );
    }

    #[test]
    fn imports_minimal_epub_archive() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("kotoclip-epub-{suffix}.epub"));
        let file = File::create(&path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        archive
            .start_file("META-INF/container.xml", options)
            .unwrap();
        archive
            .write_all(
                br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
            )
            .unwrap();
        archive.start_file("OEBPS/content.opf", options).unwrap();
        archive
            .write_all(
                r#"<?xml version="1.0"?><package xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>测试书</dc:title><dc:creator>测试作者</dc:creator><dc:date>2026-07-19</dc:date><dc:language>ja</dc:language></metadata><manifest><item id="chapter" href="text/chapter.xhtml"/></manifest><spine><itemref idref="chapter"/></spine></package>"#.as_bytes(),
            )
            .unwrap();
        archive
            .start_file("OEBPS/text/chapter.xhtml", options)
            .unwrap();
        archive
            .write_all(
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>彼は<ruby>本<rt>ほん</rt></ruby>を読む。</p></body></html>"#.as_bytes(),
            )
            .unwrap();
        archive.finish().unwrap();

        let imported = import_epub(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(imported.title, "测试书");
        assert_eq!(imported.author, "测试作者");
        assert!(imported.markdown.contains("彼は本《ほん》を読む。"));
        assert!(imported.warnings.is_empty());
    }
}
