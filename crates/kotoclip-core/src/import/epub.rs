use regex::Regex;
use roxmltree::{Document, Node};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::OnceLock;
use thiserror::Error;
use zip::ZipArchive;

const OPS_NAMESPACE: &str = "http://www.idpf.org/2007/ops";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportedEpub {
    pub source_name: String,
    pub title: String,
    pub author: String,
    pub date: String,
    pub language: String,
    pub markdown: String,
    pub chapter_titles: Vec<String>,
    pub resources: Vec<ImportedEpubResource>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportedEpubResource {
    pub href: String,
    pub media_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(skip_serializing)]
    pub bytes: Vec<u8>,
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
struct ManifestItem {
    id: String,
    href: String,
    media_type: String,
    properties: Vec<String>,
}

#[derive(Debug, Clone)]
struct SpineItem {
    manifest: ManifestItem,
    linear: bool,
}

#[derive(Debug, Clone)]
struct PackageDocument {
    manifest: HashMap<String, ManifestItem>,
    spine: Vec<SpineItem>,
    role_hints: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavigationEntry {
    title: String,
    path: String,
    fragment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentRole {
    Cover,
    Illustration,
    Toc,
    Notice,
    TitlePage,
    Backmatter,
    Content,
}

#[derive(Debug, Clone)]
struct DocumentMetrics {
    text_chars: usize,
    links: usize,
    images: usize,
    headings: usize,
}

#[derive(Debug, Clone)]
struct SpineDocument {
    path: String,
    xhtml: String,
    role: DocumentRole,
    metrics: DocumentMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CanonicalBlock {
    Heading(String),
    Paragraph(String),
    Image { href: String, alt: String },
}

#[derive(Debug, Clone)]
enum InlinePart {
    Text(String),
    Break,
    Heading(String),
    Image { href: String, alt: String },
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
    let opf = normalize_xml_source(&read_zip_text(&mut archive, &opf_path)?);
    let opf_document = Document::parse(strip_xml_declaration(&opf))?;
    let metadata = read_metadata(&opf_document, &fallback_title);
    let package = read_package(&opf_document, &opf_path)?;
    let mut warnings = Vec::new();
    let known_images = image_entries(&mut archive)?;
    let primary_navigation = read_navigation(&mut archive, &package, &opf_path, &mut warnings)?;
    let documents = read_spine_documents(
        &mut archive,
        &package,
        &opf_path,
        &primary_navigation,
        &mut warnings,
    )?;
    let navigation = merge_navigation(
        primary_navigation,
        read_spine_navigation(&documents),
        &documents,
    );
    if navigation.is_empty() {
        warnings.push("EPUB 未提供可用 nav/NCX/XHTML 目录，将使用正文结构标题".to_string());
    }
    let chapters = select_chapters(&navigation, &documents);
    let entries_by_path = chapters.iter().cloned().fold(
        HashMap::<String, Vec<NavigationEntry>>::new(),
        |mut grouped, entry| {
            grouped.entry(entry.path.clone()).or_default().push(entry);
            grouped
        },
    );

    let mut blocks = Vec::new();
    let mut referenced_images = Vec::new();
    let first_body_path = chapters.first().map(|chapter| chapter.path.as_str());
    let mut body_started = first_body_path.is_none();
    for document in &documents {
        if first_body_path.is_some_and(|path| document.path == path) {
            body_started = true;
        }
        let targets = entries_by_path
            .get(&document.path)
            .map(Vec::as_slice)
            .unwrap_or_default();
        render_spine_document(
            document,
            targets,
            &known_images,
            &mut blocks,
            &mut referenced_images,
            &mut warnings,
            body_started,
        )?;
    }
    normalize_blocks(&mut blocks);

    let chapter_titles = blocks
        .iter()
        .filter_map(|block| match block {
            CanonicalBlock::Heading(title) if !is_toc_title(title) => Some(title.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let markdown = serialize_markdown(&metadata, &chapter_titles, &blocks);
    let resources = collect_image_resources(
        &mut archive,
        &known_images,
        &referenced_images,
        &mut warnings,
    )?;

    Ok(ImportedEpub {
        source_name,
        title: metadata.title,
        author: metadata.author,
        date: metadata.date,
        language: metadata.language,
        markdown,
        chapter_titles,
        resources,
        warnings,
    })
}

fn locate_opf<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, EpubImportError> {
    if let Ok(container) = read_zip_text(archive, "META-INF/container.xml") {
        let container = normalize_xml_source(&container);
        if let Ok(document) = Document::parse(strip_xml_declaration(&container)) {
            if let Some(path) = document
                .descendants()
                .find(|node| node.has_tag_name("rootfile"))
                .and_then(|node| node.attribute("full-path"))
            {
                return Ok(normalize_archive_path(path));
            }
        }
    }
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.name().to_ascii_lowercase().ends_with(".opf") {
            return Ok(normalize_archive_path(file.name()));
        }
    }
    Err(EpubImportError::MissingOpf)
}

fn read_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, EpubImportError> {
    let actual = find_zip_entry(archive, name, name).unwrap_or_else(|| name.to_string());
    let mut file = archive.by_name(&actual)?;
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
            .map(node_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let author_id = document
        .descendants()
        .find(|node| {
            node.has_tag_name("meta")
                && node.attribute("property") == Some("role")
                && node.text().map(str::trim) == Some("aut")
        })
        .and_then(|node| node.attribute("refines"))
        .map(|value| value.trim_start_matches('#'));
    let author = document
        .descendants()
        .filter(|node| node.has_tag_name("creator"))
        .find(|node| {
            node.attribute("role") == Some("aut")
                || author_id.is_some_and(|id| node.attribute("id") == Some(id))
        })
        .or_else(|| {
            document
                .descendants()
                .find(|node| node.has_tag_name("creator"))
        })
        .map(node_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());
    let date = text("date").or_else(|| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("meta") && node.attribute("property") == Some("dcterms:modified")
            })
            .map(node_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| value.get(..10).unwrap_or(&value).to_string())
    });
    Metadata {
        title: normalize_spaced_title(&text("title").unwrap_or_else(|| fallback_title.to_string())),
        author,
        date: date.unwrap_or_else(|| "未知".to_string()),
        language: text("language").unwrap_or_else(|| "ja".to_string()),
    }
}

fn read_package(
    document: &Document<'_>,
    opf_path: &str,
) -> Result<PackageDocument, EpubImportError> {
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
        .filter_map(|node| {
            let id = node.attribute("id")?;
            let href = node.attribute("href")?;
            Some((
                id.to_string(),
                ManifestItem {
                    id: id.to_string(),
                    href: href.to_string(),
                    media_type: node.attribute("media-type").unwrap_or_default().to_string(),
                    properties: token_list(node.attribute("properties")),
                },
            ))
        })
        .collect::<HashMap<_, _>>();
    let spine = spine_node
        .children()
        .filter(|node| node.has_tag_name("itemref"))
        .filter_map(|node| {
            let item = manifest.get(node.attribute("idref")?)?.clone();
            Some(SpineItem {
                manifest: item,
                linear: node.attribute("linear") != Some("no"),
            })
        })
        .collect::<Vec<_>>();
    let mut role_hints = HashMap::<String, Vec<String>>::new();
    for reference in document
        .descendants()
        .filter(|node| node.has_tag_name("reference"))
    {
        let Some(href) = reference.attribute("href") else {
            continue;
        };
        let path = resolve_zip_path(opf_path, href);
        let hints = role_hints.entry(path).or_default();
        hints.extend(token_list(reference.attribute("type")));
        hints.extend(token_list(reference.attribute("title")));
    }
    Ok(PackageDocument {
        manifest,
        spine,
        role_hints,
    })
}

fn read_navigation<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    package: &PackageDocument,
    opf_path: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<NavigationEntry>, EpubImportError> {
    if let Some(item) = package
        .manifest
        .values()
        .find(|item| item.properties.iter().any(|value| value == "nav"))
    {
        let nav_path = resolve_zip_path(opf_path, &item.href);
        match read_zip_text(archive, &nav_path) {
            Ok(text) => {
                let text = normalize_xml_source(&text);
                match Document::parse(strip_xml_declaration(&text)) {
                    Ok(document) => {
                        let result = parse_epub3_navigation(&document, &nav_path);
                        if !result.is_empty() {
                            return Ok(result);
                        }
                        warnings.push("EPUB3 nav 不包含可用 toc，尝试 NCX".to_string());
                    }
                    Err(error) => warnings.push(format!("EPUB3 nav 解析失败，尝试 NCX：{error}")),
                }
            }
            Err(error) => warnings.push(format!("EPUB3 nav 读取失败，尝试 NCX：{error}")),
        }
    }
    if let Some(item) = package
        .manifest
        .values()
        .find(|item| item.media_type == "application/x-dtbncx+xml")
    {
        let ncx_path = resolve_zip_path(opf_path, &item.href);
        let text = normalize_xml_source(&read_zip_text(archive, &ncx_path)?);
        let document = Document::parse(strip_xml_declaration(&text))?;
        let result = parse_ncx_navigation(&document, &ncx_path);
        if !result.is_empty() {
            return Ok(result);
        }
    }
    Ok(Vec::new())
}

fn parse_epub3_navigation(document: &Document<'_>, nav_path: &str) -> Vec<NavigationEntry> {
    let toc = document
        .descendants()
        .filter(|node| node.has_tag_name("nav"))
        .find(|node| {
            token_list(epub_type(*node))
                .iter()
                .any(|value| value == "toc")
                || token_list(node.attribute("role"))
                    .iter()
                    .any(|value| value == "doc-toc")
        })
        .or_else(|| document.descendants().find(|node| node.has_tag_name("nav")));
    toc.into_iter()
        .flat_map(|node| node.descendants())
        .filter(|node| node.has_tag_name("a"))
        .filter_map(|node| navigation_entry(nav_path, node.attribute("href")?, &node_text(node)))
        .collect()
}

fn parse_ncx_navigation(document: &Document<'_>, ncx_path: &str) -> Vec<NavigationEntry> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("navPoint"))
        .filter_map(|point| {
            let href = point
                .children()
                .find(|node| node.has_tag_name("content"))?
                .attribute("src")?;
            let title = point
                .children()
                .find(|node| node.has_tag_name("navLabel"))
                .map(node_text)
                .unwrap_or_default();
            navigation_entry(ncx_path, href, &title)
        })
        .collect()
}

fn navigation_entry(base_path: &str, href: &str, title: &str) -> Option<NavigationEntry> {
    let title = normalize_spaced_title(&normalize_inline_text(title));
    if title.is_empty() || is_external_reference(href) {
        return None;
    }
    let (path, fragment) = resolve_reference(base_path, href);
    Some(NavigationEntry {
        title,
        path,
        fragment,
    })
}

fn read_spine_navigation(documents: &[SpineDocument]) -> Vec<NavigationEntry> {
    let mut entries = Vec::new();
    for source in documents {
        let Ok(document) = Document::parse(strip_xml_declaration(&source.xhtml)) else {
            continue;
        };
        let Some(body) = document
            .descendants()
            .find(|node| node.has_tag_name("body"))
        else {
            continue;
        };
        let toc_roots = if source.role == DocumentRole::Toc {
            vec![body]
        } else {
            document
                .descendants()
                .filter(|node| node.has_tag_name("nav") && is_toc_navigation_node(*node))
                .collect::<Vec<_>>()
        };
        for root in toc_roots {
            entries.extend(
                root.descendants()
                    .filter(|node| node.has_tag_name("a"))
                    .filter_map(|node| {
                        navigation_entry(&source.path, node.attribute("href")?, &node_text(node))
                    }),
            );
        }
    }
    entries
}

fn merge_navigation(
    primary: Vec<NavigationEntry>,
    secondary: Vec<NavigationEntry>,
    documents: &[SpineDocument],
) -> Vec<NavigationEntry> {
    let primary = deduplicate_navigation(primary);
    let secondary = deduplicate_navigation(secondary);
    let (preferred, fallback) = if secondary.len() > primary.len() {
        (secondary, primary)
    } else {
        (primary, secondary)
    };
    let mut merged = preferred.into_iter().chain(fallback).collect::<Vec<_>>();
    for entry in &mut merged {
        if let Some(document) = documents
            .iter()
            .find(|document| document.path.eq_ignore_ascii_case(&entry.path))
        {
            entry.path.clone_from(&document.path);
        }
    }
    deduplicate_navigation(merged)
}

fn deduplicate_navigation(entries: Vec<NavigationEntry>) -> Vec<NavigationEntry> {
    entries.into_iter().fold(Vec::new(), |mut unique, entry| {
        if let Some(index) = unique
            .iter()
            .position(|existing| navigation_equivalent(existing, &entry))
        {
            if is_structural_navigation_title(&unique[index].title)
                && !is_structural_navigation_title(&entry.title)
            {
                unique[index] = entry;
            }
        } else {
            unique.push(entry);
        }
        unique
    })
}

fn navigation_equivalent(left: &NavigationEntry, right: &NavigationEntry) -> bool {
    left.path.eq_ignore_ascii_case(&right.path)
        && (left.fragment == right.fragment || heading_equivalent(&left.title, &right.title))
}

fn is_toc_navigation_node(node: Node<'_, '_>) -> bool {
    token_list(epub_type(node))
        .iter()
        .any(|value| value == "toc")
        || token_list(node.attribute("role"))
            .iter()
            .any(|value| value == "doc-toc")
        || token_list(node.attribute("id"))
            .iter()
            .any(|value| value == "toc")
        || token_list(node.attribute("class"))
            .iter()
            .any(|value| value == "toc")
}

fn read_spine_documents<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    package: &PackageDocument,
    opf_path: &str,
    navigation: &[NavigationEntry],
    warnings: &mut Vec<String>,
) -> Result<Vec<SpineDocument>, EpubImportError> {
    let navigation_paths = navigation
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<HashSet<_>>();
    let nav_manifest_paths = package
        .manifest
        .values()
        .filter(|item| item.properties.iter().any(|value| value == "nav"))
        .map(|item| resolve_zip_path(opf_path, &item.href))
        .collect::<HashSet<_>>();
    let mut result = Vec::new();
    for spine in &package.spine {
        if !spine.linear
            && !navigation_paths.contains(resolve_zip_path(opf_path, &spine.manifest.href).as_str())
        {
            continue;
        }
        let requested = resolve_zip_path(opf_path, &spine.manifest.href);
        if nav_manifest_paths.contains(&requested) {
            continue;
        }
        let Some(path) = find_zip_entry(archive, &requested, &spine.manifest.href) else {
            warnings.push(format!("未找到正文文件：{}", spine.manifest.href));
            continue;
        };
        let xhtml = match read_zip_text(archive, &path) {
            Ok(text) => normalize_xml_source(&text),
            Err(error) => {
                warnings.push(format!("读取正文 {} 失败：{error}", spine.manifest.href));
                continue;
            }
        };
        let document = match Document::parse(strip_xml_declaration(&xhtml)) {
            Ok(document) => document,
            Err(error) => {
                warnings.push(format!("解析正文 {} 失败：{error}", spine.manifest.href));
                continue;
            }
        };
        let Some(body) = document
            .descendants()
            .find(|node| node.has_tag_name("body"))
        else {
            warnings.push(format!("正文 {} 缺少 body", spine.manifest.href));
            continue;
        };
        let metrics = document_metrics(body);
        let role = classify_document(
            body,
            &spine.manifest,
            package
                .role_hints
                .get(&requested)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            &metrics,
        );
        drop(document);
        result.push(SpineDocument {
            path: normalize_archive_path(&path),
            xhtml,
            role,
            metrics,
        });
    }
    Ok(result)
}

fn document_metrics(body: Node<'_, '_>) -> DocumentMetrics {
    DocumentMetrics {
        text_chars: normalize_inline_text(&node_text(body)).chars().count(),
        links: body
            .descendants()
            .filter(|node| node.has_tag_name("a"))
            .count(),
        images: body
            .descendants()
            .filter(|node| node.has_tag_name("img") || node.has_tag_name("image"))
            .count(),
        headings: body
            .descendants()
            .filter(|node| heading_level(*node).is_some())
            .count(),
    }
}

fn classify_document(
    body: Node<'_, '_>,
    item: &ManifestItem,
    role_hints: &[String],
    metrics: &DocumentMetrics,
) -> DocumentRole {
    let mut signals = Vec::new();
    signals.extend(item.properties.iter().cloned());
    signals.extend(role_hints.iter().cloned());
    signals.extend(token_list(epub_type(body)));
    signals.extend(token_list(body.attribute("class")));
    signals.extend(token_list(Some(&item.id)));
    signals.extend(token_list(Some(&item.href)));
    let joined = signals.join(" ").to_ascii_lowercase();
    let has = |values: &[&str]| values.iter().any(|value| joined.contains(value));

    if has(&["cover", "表紙"]) && metrics.images > 0 {
        return DocumentRole::Cover;
    }
    if (has(&["toc", "contents", "目次"]) && metrics.links >= 2)
        || (metrics.links >= 3 && metrics.text_chars < 2_000)
    {
        return DocumentRole::Toc;
    }
    if has(&["caution", "notice", "copyright"]) && metrics.text_chars < 2_000 {
        return DocumentRole::Notice;
    }
    if has(&["titlepage", "title-page"]) && metrics.text_chars < 800 {
        return DocumentRole::TitlePage;
    }
    if has(&["colophon", "奥付"]) {
        return DocumentRole::Backmatter;
    }
    if metrics.images > 0 && metrics.text_chars == 0 {
        return DocumentRole::Illustration;
    }
    DocumentRole::Content
}

fn select_chapters(
    navigation: &[NavigationEntry],
    documents: &[SpineDocument],
) -> Vec<NavigationEntry> {
    let documents = documents
        .iter()
        .map(|document| (document.path.as_str(), document))
        .collect::<HashMap<_, _>>();
    let illustration_targets = navigation
        .iter()
        .filter(|entry| {
            documents
                .get(entry.path.as_str())
                .is_some_and(|document| document.role == DocumentRole::Illustration)
                && !is_structural_navigation_title(&entry.title)
        })
        .count();
    let has_explicit_chapter_titles = navigation
        .iter()
        .any(|entry| looks_like_chapter_title(&entry.title));
    let mut started = false;
    navigation
        .iter()
        .filter_map(|entry| {
            let document = documents.get(entry.path.as_str())?;
            let visual_heading_seed = !has_explicit_chapter_titles
                && navigation_target_has_structural_heading(entry, document);
            if matches!(
                document.role,
                DocumentRole::Cover | DocumentRole::Toc | DocumentRole::Notice
            ) || is_structural_navigation_title(&entry.title)
                || (document.role == DocumentRole::TitlePage && !visual_heading_seed)
            {
                return None;
            }
            let body_seed = document.metrics.text_chars >= 500
                || document.metrics.headings > 0
                || (document.role == DocumentRole::Illustration && illustration_targets >= 2)
                || visual_heading_seed
                || looks_like_chapter_title(&entry.title);
            if !started && !body_seed {
                return None;
            }
            started = true;
            Some(entry.clone())
        })
        .collect()
}

fn navigation_target_has_structural_heading(
    entry: &NavigationEntry,
    document: &SpineDocument,
) -> bool {
    let Ok(parsed) = Document::parse(strip_xml_declaration(&document.xhtml)) else {
        return false;
    };
    let Some(body) = parsed.descendants().find(|node| node.has_tag_name("body")) else {
        return false;
    };
    let target = entry
        .fragment
        .as_deref()
        .and_then(|fragment| {
            body.descendants()
                .find(|node| node.attribute("id") == Some(fragment))
        })
        .unwrap_or(body);

    std::iter::once(target)
        .chain(target.descendants())
        .filter(|node| node.is_element() && is_structural_heading(*node))
        .any(|node| heading_equivalent(&structural_heading_text(node), &entry.title))
}

fn structural_heading_text(node: Node<'_, '_>) -> String {
    normalize_inline_text(
        &node
            .descendants()
            .filter_map(|descendant| {
                if !descendant.is_text()
                    || descendant
                        .ancestors()
                        .take_while(|ancestor| *ancestor != node)
                        .any(|ancestor| {
                            ancestor.is_element()
                                && matches!(ancestor.tag_name().name(), "rt" | "rp")
                        })
                {
                    return None;
                }
                descendant.text()
            })
            .collect::<String>(),
    )
}

fn render_spine_document(
    source: &SpineDocument,
    targets: &[NavigationEntry],
    known_images: &HashSet<String>,
    blocks: &mut Vec<CanonicalBlock>,
    referenced_images: &mut Vec<String>,
    warnings: &mut Vec<String>,
    render_text: bool,
) -> Result<(), EpubImportError> {
    if matches!(source.role, DocumentRole::Toc | DocumentRole::Notice)
        || (source.role == DocumentRole::TitlePage && targets.is_empty())
    {
        return Ok(());
    }
    let document = Document::parse(strip_xml_declaration(&source.xhtml))?;
    let Some(body) = document
        .descendants()
        .find(|node| node.has_tag_name("body"))
    else {
        return Ok(());
    };
    let start = blocks.len();
    let mut renderer = DocumentRenderer {
        document_path: &source.path,
        targets,
        emitted_targets: vec![false; targets.len()],
        known_images,
        referenced_images,
        warnings,
    };
    if render_text {
        renderer.emit_document_targets(blocks);
        append_inline_parts(blocks, renderer.target_parts(document.root_element()));
        append_inline_parts(blocks, renderer.target_parts(body));
    }
    if !render_text
        || matches!(
            source.role,
            DocumentRole::Cover | DocumentRole::Illustration
        )
    {
        for image in body
            .descendants()
            .filter(|node| node.has_tag_name("img") || node.has_tag_name("image"))
        {
            let parts = renderer.render_image(image);
            append_inline_parts(blocks, parts);
        }
    } else {
        for child in body.children() {
            renderer.render_block(child, blocks);
        }
        if source.role == DocumentRole::Backmatter
            && !blocks.iter().any(
                |block| matches!(block, CanonicalBlock::Heading(title) if is_colophon_title(title)),
            )
        {
            blocks.insert(start, CanonicalBlock::Heading("奥付".to_string()));
        }
    }
    let missing = renderer
        .targets
        .iter()
        .enumerate()
        .filter(|(index, _)| !renderer.emitted_targets[*index])
        .map(|(_, target)| target.title.clone())
        .collect::<Vec<_>>();
    for title in missing.into_iter().rev() {
        warnings.push(format!(
            "章节锚点未找到，已回退到文档开头：{}#{title}",
            source.path
        ));
        blocks.insert(start, CanonicalBlock::Heading(title));
    }
    Ok(())
}

struct DocumentRenderer<'a> {
    document_path: &'a str,
    targets: &'a [NavigationEntry],
    emitted_targets: Vec<bool>,
    known_images: &'a HashSet<String>,
    referenced_images: &'a mut Vec<String>,
    warnings: &'a mut Vec<String>,
}

impl DocumentRenderer<'_> {
    fn emit_document_targets(&mut self, blocks: &mut Vec<CanonicalBlock>) {
        for index in 0..self.targets.len() {
            if self.targets[index].fragment.is_none() {
                self.emitted_targets[index] = true;
                push_heading(blocks, self.targets[index].title.clone());
            }
        }
    }

    fn target_parts(&mut self, node: Node<'_, '_>) -> Vec<InlinePart> {
        let Some(id) = node.attribute("id") else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for index in 0..self.targets.len() {
            if !self.emitted_targets[index] && self.targets[index].fragment.as_deref() == Some(id) {
                self.emitted_targets[index] = true;
                result.push(InlinePart::Heading(self.targets[index].title.clone()));
            }
        }
        result
    }

    fn render_block(&mut self, node: Node<'_, '_>, blocks: &mut Vec<CanonicalBlock>) {
        if node.is_text() {
            let text = normalize_paragraph(node.text().unwrap_or_default());
            if !text.is_empty() {
                push_paragraph(blocks, text);
            }
            return;
        }
        if !node.is_element() {
            return;
        }
        let tag = node.tag_name().name();
        if matches!(tag, "script" | "style" | "noscript" | "nav") {
            return;
        }
        if let Some(level) = heading_level(node) {
            let mut parts = self.target_parts(node);
            let title = normalize_spaced_title(&parts_text(&self.render_inline_children(node)));
            if !title.is_empty() {
                parts.push(InlinePart::Heading(title));
            }
            append_inline_parts(blocks, parts);
            let _ = level;
            return;
        }
        if matches!(tag, "p" | "li" | "blockquote" | "dt" | "dd" | "pre") {
            let mut parts = self.target_parts(node);
            parts.extend(self.render_inline_children(node));
            append_inline_parts(blocks, parts);
            return;
        }
        if tag == "img" || tag == "image" {
            let mut parts = self.target_parts(node);
            parts.extend(self.render_image(node));
            append_inline_parts(blocks, parts);
            return;
        }
        let targets = self.target_parts(node);
        append_inline_parts(blocks, targets);
        for child in node.children() {
            self.render_block(child, blocks);
        }
    }

    fn render_inline_children(&mut self, node: Node<'_, '_>) -> Vec<InlinePart> {
        node.children()
            .flat_map(|child| self.render_inline(child))
            .collect()
    }

    fn render_inline(&mut self, node: Node<'_, '_>) -> Vec<InlinePart> {
        if node.is_text() {
            return vec![InlinePart::Text(
                node.text().unwrap_or_default().to_string(),
            )];
        }
        if !node.is_element() {
            return Vec::new();
        }
        let mut result = self.target_parts(node);
        match node.tag_name().name() {
            "script" | "style" | "noscript" | "rp" | "rt" => result,
            "br" => {
                result.push(InlinePart::Break);
                result
            }
            "ruby" => {
                result.push(InlinePart::Text(render_ruby(node)));
                result
            }
            "img" | "image" => {
                result.extend(self.render_image(node));
                result
            }
            _ => {
                result.extend(self.render_inline_children(node));
                result
            }
        }
    }

    fn render_image(&mut self, node: Node<'_, '_>) -> Vec<InlinePart> {
        let alt = node
            .attribute("alt")
            .or_else(|| node.attribute("aria-label"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let classes = token_list(node.attribute("class"));
        if classes.iter().any(|value| value == "gaiji") && !alt.is_empty() {
            return vec![InlinePart::Text(alt)];
        }
        if alt.is_empty() && is_presentation_image(&classes) {
            return Vec::new();
        }
        let Some(reference) = node
            .attribute("src")
            .or_else(|| attribute_by_local_name(node, "href"))
            .filter(|value| !value.trim().is_empty())
        else {
            return Vec::new();
        };
        if is_external_reference(reference) {
            self.warnings.push(format!("已跳过外部图片：{reference}"));
            return Vec::new();
        }
        let (href, _) = resolve_reference(self.document_path, reference);
        let key = href.to_ascii_lowercase();
        if !self.known_images.contains(&key) {
            self.warnings.push(format!("图片资源不存在：{href}"));
            return Vec::new();
        }
        if !self
            .referenced_images
            .iter()
            .any(|value| value.eq_ignore_ascii_case(&href))
        {
            self.referenced_images.push(href.clone());
        }
        vec![InlinePart::Image { href, alt }]
    }
}

fn render_ruby(node: Node<'_, '_>) -> String {
    let mut output = String::new();
    let mut base = String::new();
    for child in node.children() {
        if child.has_tag_name("rt") {
            let reading = normalize_inline_text(&node_text(child));
            let base_text = normalize_ruby_segment(&base);
            if !base_text.is_empty() {
                output.push_str(&base_text);
                if !reading.is_empty() {
                    output.push('《');
                    output.push_str(reading.trim());
                    output.push('》');
                }
            }
            base.clear();
        } else if !child.has_tag_name("rp") && !child.has_tag_name("rtc") {
            base.push_str(&plain_inline_text(child));
        }
    }
    output.push_str(&normalize_ruby_segment(&base));
    output
}

fn plain_inline_text(node: Node<'_, '_>) -> String {
    if node.is_text() {
        return node.text().unwrap_or_default().to_string();
    }
    if !node.is_element() || matches!(node.tag_name().name(), "rt" | "rp" | "script" | "style") {
        return String::new();
    }
    if node.has_tag_name("ruby") {
        return render_ruby(node);
    }
    node.children().map(plain_inline_text).collect()
}

fn append_inline_parts(blocks: &mut Vec<CanonicalBlock>, parts: Vec<InlinePart>) {
    let mut text = String::new();
    let flush = |blocks: &mut Vec<CanonicalBlock>, text: &mut String| {
        let paragraph = normalize_paragraph(text);
        text.clear();
        if !paragraph.is_empty() {
            push_paragraph(blocks, paragraph);
        }
    };
    for part in parts {
        match part {
            InlinePart::Text(value) => text.push_str(&value),
            InlinePart::Break => flush(blocks, &mut text),
            InlinePart::Heading(title) => {
                flush(blocks, &mut text);
                push_heading(blocks, title);
            }
            InlinePart::Image { href, alt } => {
                flush(blocks, &mut text);
                blocks.push(CanonicalBlock::Image { href, alt });
            }
        }
    }
    flush(blocks, &mut text);
}

fn push_heading(blocks: &mut Vec<CanonicalBlock>, title: String) {
    let title = normalize_spaced_title(&normalize_paragraph(&title));
    if title.is_empty() {
        return;
    }
    if blocks.last().is_some_and(|block| match block {
        CanonicalBlock::Heading(previous) => heading_equivalent(previous, &title),
        _ => false,
    }) {
        return;
    }
    blocks.push(CanonicalBlock::Heading(title));
}

fn push_paragraph(blocks: &mut Vec<CanonicalBlock>, paragraph: String) {
    if blocks.last().is_some_and(|block| match block {
        CanonicalBlock::Heading(title) => heading_equivalent(title, &paragraph),
        _ => false,
    }) {
        return;
    }
    blocks.push(CanonicalBlock::Paragraph(paragraph));
}

fn normalize_blocks(blocks: &mut Vec<CanonicalBlock>) {
    blocks.dedup_by(|right, left| match (&*left, &*right) {
        (CanonicalBlock::Heading(a), CanonicalBlock::Heading(b)) => heading_equivalent(a, b),
        (CanonicalBlock::Image { href: a, .. }, CanonicalBlock::Image { href: b, .. }) => {
            a.eq_ignore_ascii_case(b)
        }
        _ => false,
    });
}

fn serialize_markdown(
    metadata: &Metadata,
    chapter_titles: &[String],
    blocks: &[CanonicalBlock],
) -> String {
    let mut sections = vec![format!(
        "---\ntitle: \"{}\"\nauthor: \"{}\"\ndate: \"{}\"\nlanguage: \"{}\"\n---",
        escape_yaml(&metadata.title),
        escape_yaml(&metadata.author),
        escape_yaml(&metadata.date),
        escape_yaml(&metadata.language)
    )];
    if !chapter_titles.is_empty() {
        let mut toc = vec!["## 目次".to_string()];
        toc.extend(chapter_titles.iter().map(|title| format!("- [[#{title}]]")));
        sections.push(toc.join("\n"));
    }
    sections.extend(blocks.iter().map(|block| match block {
        CanonicalBlock::Heading(title) => format!("## {title}"),
        CanonicalBlock::Paragraph(text) => text.clone(),
        CanonicalBlock::Image { href, alt } => {
            let href = if href
                .chars()
                .any(|value| value.is_whitespace() || matches!(value, '(' | ')'))
            {
                format!("<{href}>")
            } else {
                href.clone()
            };
            format!("![{}]({href})", escape_markdown_alt(alt))
        }
    }));
    format!("{}\n", sections.join("\n\n"))
}

fn image_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<HashSet<String>, EpubImportError> {
    let mut result = HashSet::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = normalize_archive_path(file.name());
        if image_media_type(&name).is_some() {
            result.insert(name.to_ascii_lowercase());
        }
    }
    Ok(result)
}

fn collect_image_resources<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    known_images: &HashSet<String>,
    referenced_images: &[String],
    warnings: &mut Vec<String>,
) -> Result<Vec<ImportedEpubResource>, EpubImportError> {
    const MAX_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
    let mut order = referenced_images.to_vec();
    let mut all = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = normalize_archive_path(file.name());
        if known_images.contains(&name.to_ascii_lowercase()) {
            all.push(name);
        }
    }
    for name in all {
        if !order.iter().any(|value| value.eq_ignore_ascii_case(&name)) {
            order.push(name);
        }
    }
    let mut resources = Vec::new();
    let mut total_bytes = 0_u64;
    for href in order {
        let Some(actual) = find_zip_entry(archive, &href, &href) else {
            continue;
        };
        let mut file = archive.by_name(&actual)?;
        let Some(media_type) = image_media_type(&href) else {
            continue;
        };
        if file.size() > MAX_RESOURCE_BYTES || total_bytes + file.size() > MAX_TOTAL_BYTES {
            warnings.push(format!("图片资源过大，已跳过：{href}"));
            continue;
        }
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)?;
        total_bytes += bytes.len() as u64;
        let dimensions = imagesize::blob_size(&bytes).ok();
        resources.push(ImportedEpubResource {
            href,
            media_type: media_type.to_string(),
            width: dimensions.map(|size| size.width as u32),
            height: dimensions.map(|size| size.height as u32),
            bytes,
        });
    }
    Ok(resources)
}

fn image_media_type(path: &str) -> Option<&'static str> {
    let extension = path.rsplit('.').next()?.to_ascii_lowercase();
    match extension.as_str() {
        "jpeg" | "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn resolve_reference(base_path: &str, reference: &str) -> (String, Option<String>) {
    let reference = reference.trim();
    let (path, fragment) = reference
        .split_once('#')
        .map(|(path, fragment)| (path, Some(percent_decode(fragment))))
        .unwrap_or((reference, None));
    let path = path.split('?').next().unwrap_or(path);
    (
        resolve_zip_path(base_path, path),
        fragment.filter(|value| !value.is_empty()),
    )
}

fn resolve_zip_path(base_path: &str, href: &str) -> String {
    let mut segments = normalize_archive_path(base_path)
        .split('/')
        .map(str::to_string)
        .collect::<Vec<_>>();
    segments.pop();
    for segment in href.replace('\\', "/").split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            value => segments.push(percent_decode(value)),
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
    let requested = normalize_archive_path(requested);
    let normalized_href = normalize_archive_path(href);
    for index in 0..archive.len() {
        let Ok(file) = archive.by_index(index) else {
            continue;
        };
        let name = normalize_archive_path(file.name());
        if name.eq_ignore_ascii_case(&requested)
            || name
                .to_ascii_lowercase()
                .ends_with(&normalized_href.to_ascii_lowercase())
        {
            return Some(file.name().to_string());
        }
    }
    None
}

fn normalize_xml_source(input: &str) -> String {
    static NAMED_ENTITY: OnceLock<Regex> = OnceLock::new();
    let mut output = NAMED_ENTITY
        .get_or_init(|| Regex::new(r"&([A-Za-z][A-Za-z0-9]+);").unwrap())
        .replace_all(input, |captures: &regex::Captures<'_>| {
            let key = format!("{};", &captures[1]);
            let Some(&(first, second)) = html5ever::data::NAMED_ENTITIES.get(key.as_str()) else {
                return captures[0].to_string();
            };
            let mut replacement = format!("&#x{first:X};");
            if second != 0 {
                replacement.push_str(&format!("&#x{second:X};"));
            }
            replacement
        })
        .into_owned();
    for (prefix, namespace) in [
        ("epub", OPS_NAMESPACE),
        ("xlink", XLINK_NAMESPACE),
        ("opf", "http://www.idpf.org/2007/opf"),
        ("dc", "http://purl.org/dc/elements/1.1/"),
    ] {
        ensure_namespace_declaration(&mut output, prefix, namespace);
    }
    output
}

fn ensure_namespace_declaration(source: &mut String, prefix: &str, namespace: &str) {
    static ROOT: OnceLock<Regex> = OnceLock::new();
    let lower = source.to_ascii_lowercase();
    if !lower.contains(&format!("{prefix}:")) || lower.contains(&format!("xmlns:{prefix}")) {
        return;
    }
    let Some(root) = ROOT
        .get_or_init(|| {
            Regex::new(r"(?i)<(?:[A-Za-z_][A-Za-z0-9_.-]*:)?(?:html|package|ncx|container|svg)\b")
                .unwrap()
        })
        .find(source)
    else {
        return;
    };
    source.insert_str(root.end(), &format!(" xmlns:{prefix}=\"{namespace}\""));
}

fn normalize_archive_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches('/').to_string()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = |byte: u8| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            };
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_string())
}

fn strip_xml_declaration(input: &str) -> &str {
    let mut trimmed = input.trim_start_matches('\u{feff}').trim_start();
    if trimmed.starts_with("<?xml") {
        trimmed = trimmed
            .find("?>")
            .map(|end| trimmed[end + 2..].trim_start())
            .unwrap_or(trimmed);
    }
    if trimmed
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"))
    {
        trimmed = trimmed
            .find('>')
            .map(|end| trimmed[end + 1..].trim_start())
            .unwrap_or(trimmed);
    }
    trimmed
}

fn node_text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

fn attribute_by_local_name<'input>(node: Node<'input, 'input>, name: &str) -> Option<&'input str> {
    node.attributes()
        .find(|attribute| attribute.name() == name)
        .map(|attribute| attribute.value())
}

fn epub_type<'input>(node: Node<'input, 'input>) -> Option<&'input str> {
    node.attribute((OPS_NAMESPACE, "type"))
        .or_else(|| node.attribute("epub:type"))
        .or_else(|| attribute_by_local_name(node, "type"))
}

fn token_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '/' | '\\' | '.' | '_' | '-')
        })
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn heading_level(node: Node<'_, '_>) -> Option<usize> {
    match node.tag_name().name() {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

fn is_structural_heading(node: Node<'_, '_>) -> bool {
    if heading_level(node).is_some() {
        return true;
    }
    if node.attribute("role") == Some("heading") {
        return true;
    }
    let epub_types = token_list(epub_type(node));
    if epub_types
        .iter()
        .any(|value| matches!(value.as_str(), "chapter" | "part" | "heading" | "title"))
    {
        return true;
    }
    token_list(node.attribute("class"))
        .iter()
        .any(|value| matches!(value.as_str(), "chapter" | "heading" | "headline" | "title"))
}

fn normalize_spaced_title(input: &str) -> String {
    let trimmed = input.trim();
    let characters = trimmed.chars().collect::<Vec<_>>();
    let visible = characters
        .iter()
        .enumerate()
        .filter(|(_, character)| !character.is_whitespace())
        .collect::<Vec<_>>();
    if visible.len() < 3 {
        return trimmed.to_string();
    }
    let spaced_boundaries = visible
        .windows(2)
        .filter(|pair| {
            characters[pair[0].0 + 1..pair[1].0]
                .iter()
                .all(|value| value.is_whitespace())
        })
        .filter(|pair| pair[1].0 > pair[0].0 + 1)
        .count();
    if spaced_boundaries >= 2 && spaced_boundaries * 2 >= visible.len() - 1 {
        visible
            .into_iter()
            .map(|(_, character)| *character)
            .collect()
    } else {
        trimmed.to_string()
    }
}

fn normalize_inline_text(input: &str) -> String {
    let mut collapsed = String::new();
    let mut pending_space = false;
    for character in input.replace('…', "...").chars() {
        if character.is_ascii_whitespace() {
            pending_space = !collapsed.is_empty();
            continue;
        }
        if pending_space {
            let previous = collapsed.chars().next_back();
            if previous.is_some_and(|value| value.is_ascii_alphanumeric())
                && character.is_ascii_alphanumeric()
            {
                collapsed.push(' ');
            }
            pending_space = false;
        }
        collapsed.push(character);
    }
    collapsed
}

fn normalize_ruby_segment(input: &str) -> String {
    normalize_inline_text(input).trim().to_string()
}

fn normalize_paragraph(input: &str) -> String {
    normalize_inline_text(input)
        .trim_matches(|character: char| character.is_ascii_whitespace())
        .to_string()
}

fn parts_text(parts: &[InlinePart]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            InlinePart::Text(value) => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

fn heading_equivalent(left: &str, right: &str) -> bool {
    normalize_heading_key(left) == normalize_heading_key(right)
}

fn normalize_heading_key(value: &str) -> String {
    static RUBY: OnceLock<Regex> = OnceLock::new();
    static DASH: OnceLock<Regex> = OnceLock::new();
    let without_ruby = RUBY
        .get_or_init(|| Regex::new(r"《[^》]+》").unwrap())
        .replace_all(value, "");
    let normalized_dashes = DASH
        .get_or_init(|| Regex::new(r"[-‐‑‒–—―─━]+").unwrap())
        .replace_all(&without_ruby, "-");
    normalized_dashes
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn looks_like_chapter_title(title: &str) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| {
            Regex::new(
                r"(?i)(序章|終章|間章|外伝|付録|おまけ|あとがき|プロローグ|エピローグ|最終話|第[0-9０-９一二三四五六七八九十百]+[章話部]|^[0-9０-９一二三四五六七八九十百]+章|chapter|prologue|epilogue|afterword)",
            )
            .unwrap()
        })
        .is_match(title)
}

fn is_structural_navigation_title(title: &str) -> bool {
    matches!(
        normalize_heading_key(title).as_str(),
        "目次"
            | "もくじ"
            | "contents"
            | "tableofcontents"
            | "表紙"
            | "cover"
            | "扉"
            | "扉絵"
            | "titlepage"
            | "frontispiece"
            | "本文"
            | "start"
            | "beginreading"
    )
}

fn is_toc_title(title: &str) -> bool {
    matches!(
        normalize_heading_key(title).as_str(),
        "目次" | "もくじ" | "contents"
    )
}

fn is_colophon_title(title: &str) -> bool {
    matches!(normalize_heading_key(title).as_str(), "奥付" | "colophon")
}

fn is_presentation_image(classes: &[String]) -> bool {
    let has = |value: &str| classes.iter().any(|class| class == value);
    (has("gaiji") && has("line"))
        || (has("keep") && has("space"))
        || has("spacer")
        || has("separator")
}

fn is_external_reference(reference: &str) -> bool {
    let lower = reference.trim().to_ascii_lowercase();
    lower.starts_with("http:")
        || lower.starts_with("https:")
        || lower.starts_with("data:")
        || lower.starts_with("javascript:")
}

fn escape_yaml(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ")
}

fn escape_markdown_alt(value: &str) -> String {
    value.replace('\\', "\\\\").replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn renders_grouped_ruby_without_xml_layout_breaks() {
        let document = Document::parse(
            "<html><body><p>自己<ruby>紹<rt>しょう</rt>\n 介<rt>かい</rt></ruby>をする。</p></body></html>",
        )
        .unwrap();
        let ruby = document
            .descendants()
            .find(|node| node.has_tag_name("ruby"))
            .unwrap();
        assert_eq!(render_ruby(ruby), "紹《しょう》介《かい》");
    }

    #[test]
    fn normalizes_only_titles_with_repeated_inter_character_spacing() {
        assert_eq!(normalize_spaced_title("　プ ロ ロ ー グ　"), "プロローグ");
        assert_eq!(normalize_spaced_title("C O N T E N T S"), "CONTENTS");
        assert_eq!(
            normalize_spaced_title("第一章　恋人とか、ぜったいにムリ！"),
            "第一章　恋人とか、ぜったいにムリ！"
        );
    }

    #[test]
    fn treats_equivalent_dash_runs_as_the_same_heading() {
        assert!(heading_equivalent(
            "プロローグ　◎　自己紹介代わりの回想─元・天才美少女作家です",
            "　プロローグ　◎　自己紹介代わりの回想──元・天才美少女作家です",
        ));
    }

    #[test]
    fn resolves_references_relative_to_the_containing_document() {
        assert_eq!(
            resolve_zip_path("OEBPS/content.opf", "text/chapter.xhtml"),
            "OEBPS/text/chapter.xhtml"
        );
        assert_eq!(
            resolve_reference("OEBPS/text/chapter.xhtml", "../images/scene%201.png#view").0,
            "OEBPS/images/scene 1.png"
        );
    }

    #[test]
    fn normalizes_legacy_html_entities_and_missing_epub_namespace() {
        let source = normalize_xml_source(
            r#"<html><body><nav epub:type="toc"><p>A&nbsp;B</p></nav></body></html>"#,
        );
        let document = Document::parse(&source).unwrap();
        let nav = document
            .descendants()
            .find(|node| node.has_tag_name("nav"))
            .unwrap();
        assert_eq!(epub_type(nav), Some("toc"));
        assert_eq!(normalize_inline_text(&node_text(nav)), "A\u{a0}B");
    }

    #[test]
    fn accepts_authoritative_navigation_for_image_only_publications() {
        let navigation = vec![
            NavigationEntry {
                title: "笔ペンの種類と選び方".to_string(),
                path: "page-1.xhtml".to_string(),
                fragment: None,
            },
            NavigationEntry {
                title: "正しい姿勢と持ち方".to_string(),
                path: "page-2.xhtml".to_string(),
                fragment: None,
            },
        ];
        let documents = ["page-1.xhtml", "page-2.xhtml"]
            .into_iter()
            .map(|path| SpineDocument {
                path: path.to_string(),
                xhtml: String::new(),
                role: DocumentRole::Illustration,
                metrics: DocumentMetrics {
                    text_chars: 0,
                    links: 0,
                    images: 1,
                    headings: 0,
                },
            })
            .collect::<Vec<_>>();

        assert_eq!(select_chapters(&navigation, &documents), navigation);
    }

    #[test]
    fn accepts_matching_visual_heading_when_navigation_has_no_chapter_labels() {
        let navigation = vec![NavigationEntry {
            title: "俺がまだ中学生だった頃の話である。".to_string(),
            path: "chapter.xhtml".to_string(),
            fragment: Some("start".to_string()),
        }];
        let documents = vec![SpineDocument {
            path: "chapter.xhtml".to_string(),
            xhtml: r#"<html><body id="start"><div><div class="title">俺がまだ中学生だった<ruby>頃<rt>ころ</rt></ruby>の話である。</div></div></body></html>"#.to_string(),
            role: DocumentRole::TitlePage,
            metrics: DocumentMetrics {
                text_chars: 16,
                links: 0,
                images: 0,
                headings: 0,
            },
        }];

        assert_eq!(select_chapters(&navigation, &documents), navigation);
    }

    #[test]
    fn keeps_descriptive_title_when_navigation_sources_share_a_target() {
        let entries = vec![
            NavigationEntry {
                title: "扉".to_string(),
                path: "chapter.xhtml".to_string(),
                fragment: Some("start".to_string()),
            },
            NavigationEntry {
                title: "俺がまだ中学生だった頃の話である。".to_string(),
                path: "chapter.xhtml".to_string(),
                fragment: Some("start".to_string()),
            },
        ];

        assert_eq!(
            deduplicate_navigation(entries)[0].title,
            "俺がまだ中学生だった頃の話である。"
        );
    }

    #[test]
    fn explicit_chapter_labels_keep_visual_frontmatter_outside_body() {
        let navigation = vec![
            NavigationEntry {
                title: "書名".to_string(),
                path: "title.xhtml".to_string(),
                fragment: Some("title".to_string()),
            },
            NavigationEntry {
                title: "底本データ".to_string(),
                path: "source.xhtml".to_string(),
                fragment: None,
            },
            NavigationEntry {
                title: "プロローグ　本文".to_string(),
                path: "prologue.xhtml".to_string(),
                fragment: None,
            },
        ];
        let documents = [
            (
                "title.xhtml",
                r#"<html><body id="title"><div class="title">書名</div></body></html>"#,
            ),
            (
                "source.xhtml",
                "<html><body><p>底本データ</p></body></html>",
            ),
            ("prologue.xhtml", "<html><body><p>本文</p></body></html>"),
        ]
        .into_iter()
        .map(|(path, xhtml)| SpineDocument {
            path: path.to_string(),
            xhtml: xhtml.to_string(),
            role: DocumentRole::Content,
            metrics: DocumentMetrics {
                text_chars: 10,
                links: 0,
                images: 0,
                headings: 0,
            },
        })
        .collect::<Vec<_>>();

        assert_eq!(select_chapters(&navigation, &documents), navigation[2..]);
    }

    #[test]
    fn imports_epub3_navigation_clean_blocks_and_images() {
        let path = fixture_path("epub3");
        write_epub3_fixture(&path);
        let imported = import_epub(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(imported.title, "测试书");
        assert_eq!(imported.author, "测试作者");
        assert_eq!(imported.chapter_titles, vec!["第一章"]);
        assert!(imported.markdown.contains("## 目次\n- [[#第一章]]"));
        assert!(imported.markdown.contains("## 第一章"));
        assert!(imported
            .markdown
            .contains("彼は紹介《しょうかい》ではなく紹《しょう》介《かい》を読む。"));
        assert!(!imported.markdown.contains("line.png"));
        assert!(!imported.markdown.contains("space.png"));
        assert!(imported
            .markdown
            .contains("![场景](OEBPS/images/scene.png)"));
        assert!(!imported.markdown.contains("纵排提示"));
        assert!(!imported.markdown.contains("![]()"));
        assert!(imported
            .resources
            .iter()
            .any(|resource| resource.href == "OEBPS/images/cover.jpg"));
        assert!(imported
            .resources
            .iter()
            .any(|resource| resource.href == "OEBPS/images/scene.png"));
    }

    #[test]
    fn imports_epub2_ncx_chapters() {
        let path = fixture_path("epub2");
        write_epub2_fixture(&path);
        let imported = import_epub(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(imported.chapter_titles, vec!["序章", "第一章"]);
        assert!(imported.markdown.contains("## 序章"));
        assert!(imported.markdown.contains("## 第一章"));
        assert!(imported.markdown.contains("![](front.png)"));
        assert!(!imported.markdown.contains("底本データ"));
        assert!(!imported.markdown.contains("版权提示"));
        assert!(imported.warnings.is_empty());
    }

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("kotoclip-{name}-{suffix}.epub"))
    }

    fn start_file(writer: &mut ZipWriter<File>, name: &str, content: &str) {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }

    fn write_epub3_fixture(path: &Path) {
        let mut writer = ZipWriter::new(File::create(path).unwrap());
        start_file(
            &mut writer,
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
        );
        start_file(
            &mut writer,
            "OEBPS/content.opf",
            r##"<package xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>测试书</dc:title><dc:creator role="aut">测试作者</dc:creator><dc:date>2026-07-20</dc:date><dc:language>ja</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/><item id="notice" href="notice.xhtml" media-type="application/xhtml+xml"/><item id="toc" href="toc.xhtml" media-type="application/xhtml+xml"/><item id="chapter" href="text/chapter.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="cover"/><itemref idref="notice"/><itemref idref="toc"/><itemref idref="chapter"/></spine><guide><reference type="cover" href="cover.xhtml"/></guide></package>"##,
        );
        start_file(
            &mut writer,
            "OEBPS/nav.xhtml",
            r##"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/chapter.xhtml#start">第一章</a></li></ol></nav></body></html>"##,
        );
        start_file(
            &mut writer,
            "OEBPS/cover.xhtml",
            r#"<html><body class="cover"><svg><image href="images/cover.jpg"/></svg></body></html>"#,
        );
        start_file(
            &mut writer,
            "OEBPS/notice.xhtml",
            r#"<html><body class="p-caution"><p>纵排提示</p></body></html>"#,
        );
        start_file(
            &mut writer,
            "OEBPS/toc.xhtml",
            r#"<html><body class="p-toc"><p><a href="text/chapter.xhtml">第一章</a></p><p><a href="text/chapter.xhtml">重复</a></p></body></html>"#,
        );
        start_file(
            &mut writer,
            "OEBPS/text/chapter.xhtml",
            "<html><body><h1 id=\"start\">第一章</h1><p>彼は<ruby>紹介<rt>しょうかい</rt></ruby>ではなく<ruby>紹<rt>しょう</rt>\n 介<rt>かい</rt></ruby>\n を読む。</p><p><img class=\"gaiji-line\" src=\"../images/line.png\"/><img src=\"../images/scene.png\" alt=\"场景\"/><img class=\"keep-space\" src=\"../images/space.png\"/><img alt=\"无源\"/></p></body></html>",
        );
        writer
            .start_file("OEBPS/images/cover.jpg", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"cover").unwrap();
        writer
            .start_file("OEBPS/images/scene.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"scene").unwrap();
        writer
            .start_file("OEBPS/images/line.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"line").unwrap();
        writer
            .start_file("OEBPS/images/space.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"space").unwrap();
        writer.finish().unwrap();
    }

    fn write_epub2_fixture(path: &Path) {
        let mut writer = ZipWriter::new(File::create(path).unwrap());
        start_file(
            &mut writer,
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#,
        );
        start_file(
            &mut writer,
            "content.opf",
            r#"<package><metadata><title>本</title><creator>著者</creator></metadata><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="front" href="front.html"/><item id="toc-page" href="toc.html"/><item id="notice" href="notice.html"/><item id="prologue" href="prologue.html"/><item id="chapter" href="chapter.html"/></manifest><spine><itemref idref="front"/><itemref idref="toc-page"/><itemref idref="notice"/><itemref idref="prologue"/><itemref idref="chapter"/></spine></package>"#,
        );
        start_file(
            &mut writer,
            "toc.ncx",
            r#"<ncx><navMap><navPoint><navLabel><text>第一章</text></navLabel><content src="chapter.html#c1"/></navPoint></navMap></ncx>"#,
        );
        start_file(
            &mut writer,
            "front.html",
            r#"<html><body><p>底本データ</p><p><img src="front.png"/></p></body></html>"#,
        );
        start_file(
            &mut writer,
            "toc.html",
            r#"<html><body class="p-toc"><p><a href="prologue.html">序章</a></p><p><a href="chapter.html#c1">第一章</a></p></body></html>"#,
        );
        start_file(
            &mut writer,
            "notice.html",
            r#"<html><body class="copyright-notice"><p>版权提示</p></body></html>"#,
        );
        start_file(
            &mut writer,
            "prologue.html",
            &format!(
                "<html><body><p>{}</p></body></html>",
                "序章正文。".repeat(100)
            ),
        );
        start_file(
            &mut writer,
            "chapter.html",
            r#"<html><body id="c1"><p>第一章正文。</p></body></html>"#,
        );
        writer
            .start_file("front.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"front").unwrap();
        writer.finish().unwrap();
    }
}
