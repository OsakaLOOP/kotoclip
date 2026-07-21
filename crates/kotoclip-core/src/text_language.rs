use cjclassifier::{CJClassifier, CJLanguage, Results};
use std::sync::{Arc, LazyLock};

static CJK_LANGUAGE_DETECTOR: LazyLock<Option<Arc<CJClassifier>>> = LazyLock::new(|| {
    CJClassifier::load()
        .map_err(|error| log::error!("中日文字识别模型加载失败：{error}"))
        .ok()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CjkLanguage {
    ChineseSimplified,
    ChineseTraditional,
    Japanese,
    Undetermined,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CjkLanguageDetection {
    pub language: CjkLanguage,
    pub confidence_gap: f64,
    pub ideograph_count: usize,
    pub kana_count: usize,
}

impl Default for CjkLanguageDetection {
    fn default() -> Self {
        Self {
            language: CjkLanguage::Undetermined,
            confidence_gap: 0.0,
            ideograph_count: 0,
            kana_count: 0,
        }
    }
}

pub fn detect_cjk_language(value: &str) -> CjkLanguage {
    detect_cjk_language_with_evidence(value).language
}

/// 使用统一的中日文字模型返回分类及可审计证据。
///
/// 非 CJK 文本或模型加载失败时保持 `Undetermined`，调用方不得将其视为日文。
pub fn detect_cjk_language_with_evidence(value: &str) -> CjkLanguageDetection {
    let Some(detector) = CJK_LANGUAGE_DETECTOR.as_ref() else {
        return CjkLanguageDetection::default();
    };
    let mut results = Results::new();
    let language = match detector.detect_with_results(value.trim(), &mut results) {
        CJLanguage::ChineseSimplified => CjkLanguage::ChineseSimplified,
        CJLanguage::ChineseTraditional => CjkLanguage::ChineseTraditional,
        CJLanguage::Japanese => CjkLanguage::Japanese,
        CJLanguage::Unknown => CjkLanguage::Undetermined,
    };
    CjkLanguageDetection {
        language,
        confidence_gap: results.gap,
        ideograph_count: results.scores.cj_char_count.max(0) as usize,
        kana_count: results.scores.kana_count.max(0) as usize,
    }
}

pub fn is_japanese_text(value: &str) -> bool {
    detect_cjk_language(value) == CjkLanguage::Japanese
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_japanese_from_kanji_or_kana() {
        for value in ["世間話", "事務所", "人間関係", "読書", "かな", "カタカナ"] {
            assert_eq!(
                detect_cjk_language(value),
                CjkLanguage::Japanese,
                "{value}"
            );
        }
    }

    #[test]
    fn distinguishes_simplified_and_traditional_chinese() {
        for value in ["闲话", "回头看", "亚非会议", "张家长李家短"] {
            assert_eq!(
                detect_cjk_language(value),
                CjkLanguage::ChineseSimplified,
                "{value}"
            );
        }
        assert_eq!(
            detect_cjk_language("今天天氣很好，我們去公園散步"),
            CjkLanguage::ChineseTraditional
        );
    }

    #[test]
    fn keeps_text_without_cjk_evidence_undetermined() {
        for value in ["", "ABC", "123"] {
            assert_eq!(
                detect_cjk_language(value),
                CjkLanguage::Undetermined,
                "{value}"
            );
            assert!(!is_japanese_text(value));
        }
    }
}
