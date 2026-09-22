//! kyujitai.js 1.3.0 的旧字体字符映射 Rust 移植。
//! 上游数据保留在 vendor/kyujitai.js/dist/kyujitai.json；词语级 douon 规则留给后置词汇层。
use std::{collections::HashMap, sync::OnceLock};

#[derive(serde::Deserialize)]
struct KyujitaiData {
    kyuji: Vec<(String, String, Option<String>)>,
}

static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();

fn table() -> &'static HashMap<char, char> {
    TABLE.get_or_init(|| {
        let data: KyujitaiData = serde_json::from_str(include_str!("../../../vendor/kyujitai.js/dist/kyujitai.json"))
            .expect("kyujitai.js 数据无效");
        data.kyuji.into_iter().filter_map(|(modern, old, _)| {
            let modern = modern.chars().next()?;
            let old = old.chars().next()?;
            Some((old, modern))
        }).collect()
    })
}

/// 将旧字体字符转换为新字体，保持字符串长度与字符顺序。
pub fn normalize(text: &str) -> String {
    text.chars().map(|character| table().get(&character).copied().unwrap_or(character)).collect()
}

pub fn normalize_char(character: char) -> char { table().get(&character).copied().unwrap_or(character) }

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn converts_provider_blocking_old_characters() {
        assert_eq!(normalize("雜搖霽欝"), "雑揺霽欝");
    }
}
