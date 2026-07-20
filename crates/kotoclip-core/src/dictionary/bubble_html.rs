use crate::models::{DictEntry, DictionaryLink, DictionaryLookup};

const GENERIC_CSS: &str = include_str!("../../../../src/styles/dictionaries/generic.css");
const DAIJIRIN_CSS: &str = include_str!("../../../../src/styles/dictionaries/daijirin.css");
const SHOGAKUKAN_CSS: &str = include_str!("../../../../src/styles/dictionaries/shogakukan.css");
const CROWN_CSS: &str = include_str!("../../../../src/styles/dictionaries/crown.css");

/// 生成与 TooltipPanel 相同信息架构的自包含研究预览。
/// 预览保留完整 Lookup 的候选、活动词典和 occurrence 边界，不再按读音铺开所有词条。
pub fn generate_bubble_preview_html(lookup: &DictionaryLookup) -> String {
    let groups = dictionary_groups(lookup);
    let active_dictionary = groups
        .iter()
        .find(|(_, entries)| !entries.is_empty())
        .map(|(name, _)| name.as_str());
    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    html.push_str("<title>Kotoclip 词典 Lookup 预览 - ");
    html.push_str(&escape_html(&lookup.query));
    html.push_str("</title><style>");
    html.push_str(PREVIEW_CSS);
    html.push_str(GENERIC_CSS);
    html.push_str(DAIJIRIN_CSS);
    html.push_str(SHOGAKUKAN_CSS);
    html.push_str(CROWN_CSS);
    html.push_str("</style></head><body><main class=\"preview-shell\">");

    render_query_summary(lookup, &mut html);
    html.push_str("<section class=\"tooltip-panel-mock\" aria-label=\"词典气泡预览\">");
    if lookup.entries.is_empty() {
        html.push_str("<div class=\"empty-state\">未找到满足当前质量门的词条。</div>");
    } else {
        render_candidate_bar(lookup, &mut html);
        render_dictionary_bar(&groups, active_dictionary, &mut html);
        html.push_str("<div class=\"preview-dictionary-stage\">");
        for (dictionary_name, entries) in &groups {
            if entries.is_empty() {
                continue;
            }
            let active = active_dictionary == Some(dictionary_name.as_str());
            render_dictionary_pane(lookup, dictionary_name, entries, active, &mut html);
        }
        html.push_str("</div>");
    }
    html.push_str("</section></main>");
    html.push_str(PREVIEW_SCRIPT);
    html.push_str("</body></html>");
    html
}

fn render_query_summary(lookup: &DictionaryLookup, html: &mut String) {
    html.push_str("<header class=\"preview-query\"><div><h1>");
    html.push_str(&escape_html(&lookup.query));
    html.push_str("</h1><div class=\"preview-query-meta\">");
    if let Some(reading) = &lookup.reading {
        html.push_str("<span>请求读音 ");
        html.push_str(&escape_html(reading));
        html.push_str("</span>");
    }
    html.push_str("<span>模式 ");
    html.push_str(&escape_html(&lookup.mode));
    html.push_str("</span><span>");
    html.push_str(&lookup.entries.len().to_string());
    html.push_str(" 个 occurrence</span><span>");
    html.push_str(&lookup.dictionary_names.len().to_string());
    html.push_str(" 本词典</span></div></div>");
    if let Some(timing) = &lookup.timing {
        html.push_str("<div class=\"preview-timing\"><strong>");
        html.push_str(&timing.service_ms.to_string());
        html.push_str(" ms</strong><span>SQLite ");
        html.push_str(&timing.sqlite_ms.to_string());
        html.push_str(" · 释义 ");
        html.push_str(&timing.definition_ms.to_string());
        html.push_str(" · 适配 ");
        html.push_str(&timing.presentation_ms.to_string());
        html.push_str("</span></div>");
    }
    html.push_str("</header>");
}

fn render_candidate_bar(lookup: &DictionaryLookup, html: &mut String) {
    if lookup.candidates.is_empty() {
        return;
    }
    html.push_str("<section class=\"preview-choice-bar\"><div class=\"preview-choice-label\">表记候选</div><div class=\"preview-choice-options\">");
    for candidate in &lookup.candidates {
        let active = lookup.selected_target.as_deref() == Some(candidate.target.as_str());
        html.push_str("<button type=\"button\" class=\"preview-choice candidate-choice");
        if active {
            html.push_str(" active");
        }
        html.push_str("\" data-target=\"");
        html.push_str(&escape_attr(&candidate.target));
        html.push_str("\" data-dictionaries=\"");
        html.push_str(&escape_attr(&candidate.dictionary_names.join("\u{1f}")));
        html.push_str("\" title=\"");
        html.push_str(&escape_attr(&candidate.target));
        html.push_str("\">");
        html.push_str(&escape_html(if candidate.label.is_empty() {
            &candidate.target
        } else {
            &candidate.label
        }));
        html.push_str("</button>");
    }
    html.push_str("</div><div class=\"candidate-target\">");
    if let Some(target) = &lookup.selected_target {
        html.push_str("当前：");
        html.push_str(&escape_html(target));
    } else {
        html.push_str("未选择候选；保持原查询 occurrence");
    }
    html.push_str("</div></section>");
}

fn render_dictionary_bar(
    groups: &[(String, Vec<&DictEntry>)],
    active_dictionary: Option<&str>,
    html: &mut String,
) {
    html.push_str("<section class=\"preview-choice-bar dictionary-bar\"><div class=\"preview-choice-label\">词典</div><div class=\"preview-choice-options\">");
    for (name, entries) in groups {
        html.push_str("<button type=\"button\" class=\"preview-choice dictionary-choice");
        if active_dictionary == Some(name.as_str()) {
            html.push_str(" active");
        }
        if entries.is_empty() {
            html.push_str(" unavailable");
        }
        html.push_str("\" data-dictionary=\"");
        html.push_str(&escape_attr(name));
        html.push_str("\">");
        html.push_str(&escape_html(name));
        if entries.is_empty() {
            html.push_str(" · 无当前释义");
        }
        html.push_str("</button>");
    }
    html.push_str("</div></section>");
}

fn render_dictionary_pane(
    lookup: &DictionaryLookup,
    dictionary_name: &str,
    entries: &[&DictEntry],
    active: bool,
    html: &mut String,
) {
    let selected_id = default_entry(lookup, entries).map(|entry| entry.occurrence_id.as_str());
    let unresolved = entries.len() > 1 && !entries.iter().any(|entry| entry.is_preferred);
    html.push_str("<section class=\"preview-dictionary-pane");
    if active {
        html.push_str(" active");
    }
    html.push_str("\" data-dictionary-pane=\"");
    html.push_str(&escape_attr(dictionary_name));
    html.push_str("\">");

    if entries.len() > 1 {
        html.push_str("<section class=\"preview-choice-bar occurrence-bar\"><div class=\"preview-choice-label\">词条");
        if unresolved {
            html.push_str("<span class=\"ambiguity-badge\">未消歧</span>");
        }
        html.push_str("</div><div class=\"preview-choice-options\">");
        for entry in entries {
            html.push_str("<button type=\"button\" class=\"preview-choice occurrence-choice");
            if selected_id == Some(entry.occurrence_id.as_str()) {
                html.push_str(" active");
            }
            html.push_str("\" data-occurrence=\"");
            html.push_str(&escape_attr(&entry.occurrence_id));
            html.push_str("\" title=\"");
            html.push_str(&escape_attr(&occurrence_title(entry)));
            html.push_str("\">");
            if entry.is_preferred {
                html.push_str("★ ");
            }
            html.push_str(&escape_html(&occurrence_label(entry, entries)));
            html.push_str("</button>");
        }
        html.push_str("</div></section>");
    }

    for entry in entries {
        let entry_active = selected_id == Some(entry.occurrence_id.as_str());
        render_entry(entry, unresolved, entry_active, html);
    }
    html.push_str("</section>");
}

fn render_entry(entry: &DictEntry, unresolved: bool, active: bool, html: &mut String) {
    html.push_str("<article class=\"preview-entry");
    if active {
        html.push_str(" active");
    }
    html.push_str("\" data-occurrence-pane=\"");
    html.push_str(&escape_attr(&entry.occurrence_id));
    html.push_str("\">");
    render_entry_header(entry, unresolved, html);
    html.push_str(
        "<div class=\"preview-entry-body\"><div class=\"dictionary-content dictionary-content--",
    );
    html.push_str(&escape_attr(&entry.style_profile));
    html.push_str("\">");
    if !entry.definition_html.trim().is_empty() {
        html.push_str(&entry.definition_html);
    } else {
        for block in &entry.content_blocks {
            html.push_str("<section class=\"dictionary-module dictionary-module--");
            html.push_str(&escape_attr(&block.kind));
            html.push_str("\">");
            if let Some(label) = &block.label {
                html.push_str("<h4 class=\"dictionary-module__label\">");
                html.push_str(&escape_html(label));
                html.push_str("</h4>");
            }
            html.push_str("<div class=\"dictionary-module__body\">");
            html.push_str(&block.html);
            html.push_str("</div></section>");
        }
    }
    html.push_str("</div>");
    render_entry_relations(entry, html);
    render_diagnostics(entry, html);
    if let Some(raw) = &entry.raw_definition {
        html.push_str("<details class=\"preview-raw\"><summary>原始 HTML</summary><pre><code>");
        html.push_str(&escape_html(raw));
        html.push_str("</code></pre></details>");
    }
    html.push_str("</div></article>");
}

fn render_entry_header(entry: &DictEntry, unresolved: bool, html: &mut String) {
    let header = &entry.header;
    html.push_str("<header class=\"preview-entry-header\"><div class=\"preview-headword-block\"><div class=\"preview-headword-line\"><span class=\"preview-headword\">");
    html.push_str(&escape_html(if header.display_form.is_empty() {
        &entry.headword
    } else {
        &header.display_form
    }));
    html.push_str("</span>");
    if let Some(reading) = header.reading.as_ref().or(entry.reading.as_ref()) {
        html.push_str("<span class=\"preview-reading\">【");
        html.push_str(&escape_html(reading));
        html.push_str("】</span>");
    }
    html.push_str("</div><div class=\"preview-header-tags\">");
    for tag in header.pos_tags.iter().chain(header.usage_tags.iter()) {
        html.push_str("<span class=\"preview-tag\" data-kind=\"");
        html.push_str(&escape_attr(&tag.kind));
        html.push_str("\">");
        html.push_str(&escape_html(&tag.label));
        html.push_str("</span>");
    }
    if entry.entry_kind != "lexical" {
        html.push_str("<span class=\"preview-tag\" data-kind=\"entry-kind\">");
        html.push_str(entry_kind_label(&entry.entry_kind));
        html.push_str("</span>");
    }
    if let Some(hint) = entry
        .match_evidence
        .as_ref()
        .and_then(|item| match_hint(&item.kind))
    {
        html.push_str("<span class=\"preview-tag\" data-kind=\"match\">");
        html.push_str(hint);
        html.push_str("</span>");
    }
    if unresolved {
        html.push_str("<span class=\"preview-tag ambiguity-badge\">候选未消歧</span>");
    }
    html.push_str("</div></div>");

    let facts = header_facts(entry);
    if !facts.is_empty() {
        html.push_str("<div class=\"preview-header-facts\">");
        for fact in facts {
            html.push_str("<div>");
            html.push_str(&escape_html(&fact));
            html.push_str("</div>");
        }
        html.push_str("</div>");
    }
    html.push_str("</header>");
}

fn render_entry_relations(entry: &DictEntry, html: &mut String) {
    let links = entry
        .links
        .iter()
        .filter(|link| link.relation != "candidate")
        .collect::<Vec<_>>();
    if links.is_empty() {
        return;
    }
    html.push_str("<div class=\"preview-relations\">");
    for link in links {
        render_relation(link, html);
    }
    html.push_str("</div>");
}

fn render_relation(link: &DictionaryLink, html: &mut String) {
    html.push_str("<span class=\"preview-relation\" data-relation=\"");
    html.push_str(&escape_attr(&link.relation));
    html.push_str("\"><small>");
    html.push_str(relation_label(&link.relation));
    html.push_str("</small>");
    html.push_str(&escape_html(if link.label.is_empty() {
        &link.target
    } else {
        &link.label
    }));
    html.push_str("</span>");
}

fn render_diagnostics(entry: &DictEntry, html: &mut String) {
    let diagnostics = &entry.adapter_diagnostics;
    if diagnostics.warnings.is_empty() && diagnostics.omitted.is_empty() {
        return;
    }
    html.push_str("<details class=\"preview-diagnostics\"><summary>适配诊断 · ");
    html.push_str(&escape_html(&diagnostics.coverage));
    html.push_str("</summary><ul>");
    for warning in &diagnostics.warnings {
        html.push_str("<li class=\"warning\">");
        html.push_str(&escape_html(warning));
        html.push_str("</li>");
    }
    for omitted in &diagnostics.omitted {
        html.push_str("<li>省略：");
        html.push_str(&escape_html(omitted));
        html.push_str("</li>");
    }
    html.push_str("</ul></details>");
}

fn dictionary_groups(lookup: &DictionaryLookup) -> Vec<(String, Vec<&DictEntry>)> {
    let mut names = lookup.dictionary_names.clone();
    for entry in &lookup.entries {
        if !names.contains(&entry.dict_name) {
            names.push(entry.dict_name.clone());
        }
    }
    names
        .into_iter()
        .map(|name| {
            let entries = lookup
                .entries
                .iter()
                .filter(|entry| entry.dict_name == name)
                .collect::<Vec<_>>();
            (name, entries)
        })
        .collect()
}

fn default_entry<'a>(
    lookup: &DictionaryLookup,
    entries: &[&'a DictEntry],
) -> Option<&'a DictEntry> {
    if let Some(selected) = lookup.selected_occurrence_id.as_deref() {
        if let Some(entry) = entries
            .iter()
            .copied()
            .find(|entry| entry.occurrence_id == selected)
        {
            return Some(entry);
        }
    }
    let preferred = entries
        .iter()
        .copied()
        .filter(|entry| entry.is_preferred)
        .collect::<Vec<_>>();
    if preferred.len() == 1 {
        return preferred.first().copied();
    }
    entries.first().copied()
}

fn occurrence_label(entry: &DictEntry, peers: &[&DictEntry]) -> String {
    let form = if entry.header.display_form.is_empty() {
        &entry.headword
    } else {
        &entry.header.display_form
    };
    let same_identity = peers
        .iter()
        .copied()
        .filter(|peer| same_occurrence_identity(entry, peer))
        .collect::<Vec<_>>();
    if same_identity.len() > 1 {
        if let Some(discriminator) = occurrence_discriminator(entry) {
            let unique = same_identity
                .iter()
                .filter(|peer| {
                    occurrence_discriminator(peer).as_deref() == Some(discriminator.as_str())
                })
                .count()
                == 1;
            if unique {
                return format!("{form} · {discriminator}");
            }
        }
        let position = same_identity
            .iter()
            .position(|peer| peer.occurrence_id == entry.occurrence_id)
            .unwrap_or(0);
        return format!("{form} · 同形条目 {}/{}", position + 1, same_identity.len());
    }
    if entry.entry_kind != "lexical" {
        return format!("{form} · {}", entry_kind_label(&entry.entry_kind));
    }
    form.to_string()
}

fn same_occurrence_identity(left: &DictEntry, right: &DictEntry) -> bool {
    let left_form = if left.header.display_form.is_empty() {
        &left.headword
    } else {
        &left.header.display_form
    };
    let right_form = if right.header.display_form.is_empty() {
        &right.headword
    } else {
        &right.header.display_form
    };
    left_form == right_form
        && left.header.reading.as_ref().or(left.reading.as_ref())
            == right.header.reading.as_ref().or(right.reading.as_ref())
}

fn occurrence_discriminator(entry: &DictEntry) -> Option<String> {
    let mut labels = entry
        .header
        .pos_tags
        .iter()
        .chain(&entry.header.usage_tags)
        .map(|tag| tag.label.as_str())
        .filter(|label| !label.is_empty())
        .take(2)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if labels.is_empty() && entry.entry_kind != "lexical" {
        labels.push(entry_kind_label(&entry.entry_kind).to_string());
    }
    (!labels.is_empty()).then(|| labels.join(" / "))
}

fn occurrence_title(entry: &DictEntry) -> String {
    let reading = entry
        .header
        .reading
        .as_ref()
        .or(entry.reading.as_ref())
        .map(|value| format!("读音：{value}"))
        .unwrap_or_default();
    let evidence = entry
        .match_evidence
        .as_ref()
        .map(|value| {
            format!(
                "命中：{} / POS：{} / 分数：{}",
                value.kind, value.pos_match, value.score
            )
        })
        .unwrap_or_default();
    [
        reading,
        entry_kind_label(&entry.entry_kind).to_string(),
        evidence,
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("；")
}

fn header_facts(entry: &DictEntry) -> Vec<String> {
    let header = &entry.header;
    let mut facts = header
        .pronunciations
        .iter()
        .map(|item| format!("{} {}", item.label, item.value))
        .collect::<Vec<_>>();
    if let Some(origin) = &header.origin {
        facts.push(format!("词源 {origin}"));
    }
    if let Some(reading) = &header.historical_reading {
        facts.push(format!("历史读音 {reading}"));
    }
    for form in &header.scoped_forms {
        if form.form != header.display_form {
            let reading = form
                .reading
                .as_ref()
                .map(|value| format!("【{value}】"))
                .unwrap_or_default();
            facts.push(format!("异表记 {}{}", form.form, reading));
        }
    }
    if let Some(note) = &header.short_note {
        facts.push(note.clone());
    }
    if let Some(evidence) = &entry.match_evidence {
        facts.push(format!(
            "证据 {} · 读音 {} · POS {} · {}",
            evidence.kind, evidence.reading_match, evidence.pos_match, evidence.score
        ));
    }
    facts
}

fn match_hint(kind: &str) -> Option<&'static str> {
    match kind {
        "explicit_alias" => Some("词典别名"),
        "compatibility_alias" => Some("兼容表记"),
        "reading_fallback" => Some("读音回退"),
        "fuzzy" => Some("模糊命中"),
        _ => None,
    }
}

fn entry_kind_label(kind: &str) -> &'static str {
    match kind {
        "phrase" => "短语",
        "surname" => "姓氏",
        "kanji" => "汉字条",
        "prefix" => "接头成分",
        "suffix" => "接尾成分",
        "bound_morpheme" => "拘束成分",
        "onomatopoeia" => "拟声拟态",
        "navigation" => "导航",
        "redirect" => "跳转",
        _ => "词汇",
    }
}

fn relation_label(relation: &str) -> &'static str {
    match relation {
        "antonym" => "反义",
        "synonym" => "近义",
        "parent" => "亲项",
        "child" => "子项",
        "phrase" => "惯用",
        "reference" | "internal_reference" => "参照",
        "redirect" => "转至",
        _ => "关联",
    }
}

fn escape_attr(value: &str) -> String {
    escape_html(value).replace('`', "&#96;")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const PREVIEW_CSS: &str = r#"
:root {
  color-scheme: light dark;
  --bg-primary: #fbfaf7;
  --bg-secondary: #f4f1eb;
  --bg-card: #fffefa;
  --glass-bg: color-mix(in srgb, var(--bg-card) 94%, transparent);
  --glass-border: #ded9ce;
  --border-color: #ddd8cd;
  --text-primary: #22252b;
  --text-secondary: #4c5360;
  --text-muted: #7d8490;
  --accent-color: #31568c;
  --accent-light: #eaf0f8;
  --novelty-high-text: #a33b3b;
  --font-ui: "Segoe UI", "Microsoft YaHei UI", sans-serif;
  --font-ja: "Yu Gothic UI", "Yu Gothic", "Noto Sans JP", sans-serif;
  --font-zh: "Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif;
}
* { box-sizing: border-box; }
body { margin: 0; padding: 28px 16px 48px; background: var(--bg-primary); color: var(--text-primary); font-family: var(--font-ui); }
button { font: inherit; }
.preview-shell { width: min(680px, 100%); margin: 0 auto; }
.preview-query { display: flex; justify-content: space-between; gap: 18px; align-items: start; margin-bottom: 14px; padding: 0 3px; }
.preview-query h1 { margin: 0; color: var(--accent-color); font: 750 1.25rem/1.35 var(--font-ja); }
.preview-query-meta { display: flex; flex-wrap: wrap; gap: 5px 12px; margin-top: 4px; color: var(--text-muted); font-size: .72rem; }
.preview-timing { display: grid; justify-items: end; color: var(--text-muted); font-size: .68rem; }
.preview-timing strong { color: var(--text-secondary); font-size: .82rem; }
.tooltip-panel-mock { overflow: hidden; border: 1px solid var(--glass-border); border-radius: 15px; background: var(--glass-bg); box-shadow: 0 18px 48px rgba(35, 42, 54, .14); }
.preview-choice-bar { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 9px; align-items: start; padding: 9px 13px; border-bottom: 1px solid var(--border-color); }
.preview-choice-label { display: flex; align-items: center; gap: 5px; padding-top: 5px; color: var(--text-muted); font: 700 .68rem var(--font-ui); white-space: nowrap; }
.preview-choice-options { display: flex; gap: 6px; overflow-x: auto; padding-bottom: 2px; }
.preview-choice { flex: 0 0 auto; min-height: 28px; max-width: 230px; overflow: hidden; border: 1px solid var(--border-color); border-radius: 999px; padding: 3px 10px; background: var(--bg-card); color: var(--accent-color); text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
.preview-choice:hover, .preview-choice.active { border-color: var(--accent-color); background: var(--accent-light); }
.preview-choice.unavailable { color: var(--text-muted); opacity: .5; }
.candidate-target { grid-column: 2; color: var(--text-muted); font-size: .66rem; }
.preview-dictionary-pane, .preview-entry { display: none; }
.preview-dictionary-pane.active, .preview-entry.active { display: block; }
.occurrence-bar { background: color-mix(in srgb, var(--bg-secondary) 55%, transparent); }
.ambiguity-badge { color: #9a602c !important; background: #fff0d9 !important; border-color: #e8c18e !important; }
.preview-entry-header { position: sticky; top: 0; z-index: 2; display: grid; grid-template-columns: minmax(0, .9fr) minmax(170px, 1.1fr); gap: 13px; padding: 14px 16px 11px; border-bottom: 1px solid var(--border-color); background: color-mix(in srgb, var(--bg-card) 94%, transparent); backdrop-filter: blur(15px); }
.preview-headword-line { display: flex; flex-wrap: wrap; gap: 3px 5px; align-items: baseline; }
.preview-headword { color: var(--accent-color); font: 750 1.3rem/1.35 var(--font-ja); }
.preview-reading { color: var(--text-muted); font: .78rem var(--font-ja); }
.preview-header-tags { display: flex; flex-wrap: wrap; gap: 4px 6px; margin-top: 3px; }
.preview-tag { display: inline-flex; align-items: center; border: 1px solid var(--border-color); border-radius: 4px; padding: 0 5px; color: var(--text-secondary); font: 700 .64rem/1.55 var(--font-ui); }
.preview-tag[data-kind="entry-kind"], .preview-tag[data-kind="match"] { color: var(--accent-color); background: var(--accent-light); }
.preview-header-facts { display: grid; gap: 3px; padding-left: 12px; border-left: 1px solid var(--border-color); color: var(--text-secondary); font: .68rem/1.4 var(--font-ui); }
.preview-entry-body { padding: 14px 16px 16px; }
.preview-relations { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 12px; padding-top: 9px; border-top: 1px dotted var(--border-color); }
.preview-relation { color: var(--accent-color); font-size: .72rem; }
.preview-relation small { margin-right: 4px; color: var(--text-muted); }
.preview-diagnostics, .preview-raw { margin-top: 12px; color: var(--text-muted); font-size: .7rem; }
.preview-diagnostics summary, .preview-raw summary { cursor: pointer; }
.preview-diagnostics ul { margin: 6px 0 0; padding-left: 1.3rem; }
.preview-diagnostics .warning { color: var(--novelty-high-text); }
.preview-raw pre { max-height: 320px; overflow: auto; padding: 10px; border-radius: 7px; background: var(--bg-secondary); white-space: pre-wrap; overflow-wrap: anywhere; }
.empty-state { padding: 30px 18px; color: var(--text-muted); text-align: center; }
.sense-main:has(> .sense-content:first-child) { grid-template-columns: minmax(0, 1fr); }
@media (prefers-color-scheme: dark) {
  :root { --bg-primary: #17191d; --bg-secondary: #202329; --bg-card: #1d2025; --glass-border: #343941; --border-color: #343941; --text-primary: #edf0f4; --text-secondary: #c5cbd3; --text-muted: #9199a5; --accent-color: #91add4; --accent-light: #26364d; }
  .ambiguity-badge { color: #e6bd8b !important; background: #3d2c1c !important; border-color: #6c4c2b !important; }
}
@media (max-width: 500px) {
  body { padding: 12px 8px 30px; }
  .preview-query { display: grid; }
  .preview-timing { justify-items: start; }
  .preview-entry-header { grid-template-columns: minmax(0, 1fr); }
  .preview-header-facts { padding: 7px 0 0; border-top: 1px solid var(--border-color); border-left: 0; }
}
"#;

const PREVIEW_SCRIPT: &str = r#"
<script>
(() => {
  const setDictionary = (name) => {
    document.querySelectorAll('.dictionary-choice').forEach((button) => {
      button.classList.toggle('active', button.dataset.dictionary === name);
    });
    document.querySelectorAll('[data-dictionary-pane]').forEach((pane) => {
      pane.classList.toggle('active', pane.dataset.dictionaryPane === name);
    });
    document.querySelectorAll('.candidate-choice').forEach((button) => {
      const names = (button.dataset.dictionaries || '').split('\u001f');
      button.classList.toggle('unavailable', !names.includes(name));
    });
  };
  document.querySelectorAll('.dictionary-choice:not(.unavailable)').forEach((button) => {
    button.addEventListener('click', () => setDictionary(button.dataset.dictionary));
  });
  document.querySelectorAll('.occurrence-choice').forEach((button) => {
    button.addEventListener('click', () => {
      const pane = button.closest('[data-dictionary-pane]');
      pane.querySelectorAll('.occurrence-choice').forEach((item) => item.classList.toggle('active', item === button));
      pane.querySelectorAll('[data-occurrence-pane]').forEach((item) => item.classList.toggle('active', item.dataset.occurrencePane === button.dataset.occurrence));
    });
  });
  document.querySelectorAll('.candidate-choice').forEach((button) => {
    button.addEventListener('click', () => {
      document.querySelectorAll('.candidate-choice').forEach((item) => item.classList.toggle('active', item === button));
      const target = document.querySelector('.candidate-target');
      if (target) target.textContent = `静态预览候选：${button.dataset.target}（应用中选择后会重新查询）`;
    });
  });
  document.querySelectorAll('[data-example-browser]').forEach((browser) => {
    const items = Array.from(browser.querySelectorAll('[data-example-index]'));
    if (items.length <= 2) return;
    const previous = browser.querySelector('[data-example-previous]');
    const next = browser.querySelector('[data-example-next]');
    const viewport = browser.querySelector('.example-browser__viewport');
    const toggle = browser.querySelector('[data-example-toggle]');
    const counter = browser.querySelector('[data-example-counter]');
    const total = browser.querySelector('[data-example-total]');
    const pages = Math.ceil(items.length / 2);
    let page = 0;
    let expanded = false;
    const render = () => {
      browser.classList.add('is-changing');
      items.forEach((item, index) => {
        item.hidden = !expanded && Math.floor(index / 2) !== page;
      });
      previous.disabled = expanded || page === 0;
      next.disabled = expanded || page === pages - 1;
      counter.textContent = `${page + 1}/${pages}`;
      counter.hidden = expanded;
      total.hidden = !expanded;
      total.textContent = `共 ${items.length} 条`;
      browser.classList.toggle('is-expanded', expanded);
      if (expanded || previous.disabled) previous.classList.remove('is-visible');
      if (expanded || next.disabled) next.classList.remove('is-visible');
      toggle.textContent = expanded ? '折叠' : '展开';
      toggle.setAttribute('aria-expanded', String(expanded));
      window.setTimeout(() => browser.classList.remove('is-changing'), 180);
    };
    previous.addEventListener('click', () => { if (page > 0) { page -= 1; render(); } });
    next.addEventListener('click', () => { if (page < pages - 1) { page += 1; render(); } });
    viewport.addEventListener('pointermove', (event) => {
      if (expanded || event.pointerType === 'touch') return;
      const bounds = viewport.getBoundingClientRect();
      const activationWidth = Math.min(72, Math.max(48, bounds.width * 0.16));
      const offset = event.clientX - bounds.left;
      previous.classList.toggle('is-visible', !previous.disabled && offset <= activationWidth);
      next.classList.toggle('is-visible', !next.disabled && offset >= bounds.width - activationWidth);
    });
    viewport.addEventListener('pointerleave', () => {
      previous.classList.remove('is-visible');
      next.classList.remove('is-visible');
    });
    toggle.addEventListener('click', () => { expanded = !expanded; render(); });
    render();
  });
  const active = document.querySelector('.dictionary-choice.active');
  if (active) setDictionary(active.dataset.dictionary);
})();
</script>
"#;

#[cfg(test)]
mod tests {
    use super::occurrence_label;
    use crate::models::{
        DictEntry, DictionaryAdapterDiagnostics, DictionaryOccurrenceHeader, DictionarySense,
        DictionaryText,
    };

    fn daijirin_occurrence(id: &str, definition: &str) -> DictEntry {
        DictEntry {
            entry_key: id.to_string(),
            dict_name: "三省堂Super大辞林3.1".to_string(),
            headword: "立つ".to_string(),
            reading: Some("たつ".to_string()),
            is_preferred: false,
            definition_html: String::new(),
            style_profile: "daijirin".to_string(),
            content_blocks: Vec::new(),
            match_type: "exact".to_string(),
            links: Vec::new(),
            occurrence_id: id.to_string(),
            source_record_index: 0,
            entry_kind: "lexical".to_string(),
            header: DictionaryOccurrenceHeader {
                display_form: "立つ".to_string(),
                reading: Some("たつ".to_string()),
                ..DictionaryOccurrenceHeader::default()
            },
            senses: vec![DictionarySense {
                sense_id: "1".to_string(),
                definitions: vec![DictionaryText {
                    lang: "ja".to_string(),
                    qualifier: None,
                    html: definition.to_string(),
                }],
                ..DictionarySense::default()
            }],
            sections: Vec::new(),
            adapter_diagnostics: DictionaryAdapterDiagnostics::default(),
            match_evidence: None,
            raw_definition: None,
        }
    }

    #[test]
    fn occurrence_labels_never_use_sense_body_as_identity() {
        let first = daijirin_occurrence(
            "三省堂Super大辞林3.1\u{1f}128321",
            "座ったり横になったりしていた人が足を伸ばして自分の体を垂直の姿勢にする。",
        );
        let second =
            daijirin_occurrence("三省堂Super大辞林3.1\u{1f}128322", "和船で，各種の柱の称。");
        let peers = vec![&first, &second];

        let first_label = occurrence_label(&first, &peers);
        let second_label = occurrence_label(&second, &peers);

        assert_eq!(first_label, "立つ · 同形条目 1/2");
        assert_eq!(second_label, "立つ · 同形条目 2/2");
        assert!(!first_label.contains("座ったり"));
        assert!(!second_label.contains("和船"));
    }
}
