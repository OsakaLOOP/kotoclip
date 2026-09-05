use serde::{Deserialize, Serialize};

use crate::alignment::{compare_tokenizations, ProviderDisagreement};
use crate::{ProviderToken, VibratoProvider};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextRegister {
    Written,
    Spoken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedAnalysis {
    pub selected_register: TextRegister,
    pub selected_provider_id: String,
    pub selected_tokens: Vec<ProviderToken>,
    pub written_tokens: Vec<ProviderToken>,
    pub spoken_tokens: Vec<ProviderToken>,
    pub disagreements: Vec<ProviderDisagreement>,
}

pub struct ProviderRouter {
    pub written: VibratoProvider,
    pub spoken: VibratoProvider,
}

impl ProviderRouter {
    pub fn analyze(&self, text: &str) -> RoutedAnalysis {
        let written_tokens = self.written.analyze(text);
        let spoken_tokens = self.spoken.analyze(text);
        let selected_register = if looks_spoken(text) {
            TextRegister::Spoken
        } else {
            TextRegister::Written
        };
        let (selected_provider_id, selected_tokens) = match selected_register {
            TextRegister::Written => (self.written.manifest().provider_id.clone(), written_tokens.clone()),
            TextRegister::Spoken => (self.spoken.manifest().provider_id.clone(), spoken_tokens.clone()),
        };
        RoutedAnalysis {
            selected_register,
            selected_provider_id,
            selected_tokens,
            disagreements: compare_tokenizations(&written_tokens, &spoken_tokens),
            written_tokens,
            spoken_tokens,
        }
    }
}

fn looks_spoken(text: &str) -> bool {
    ["じゃん", "だよ", "だね", "かな", "って", "ねえ", "おい", "！", "!", "？", "?"]
        .iter()
        .any(|marker| text.contains(marker))
}

#[cfg(test)]
mod tests {
    #[test]
    fn register_detection_is_conservative() {
        assert_eq!(super::looks_spoken("これは説明文です。"), false);
        assert_eq!(super::looks_spoken("めっちゃすごいじゃん。"), true);
    }
}
