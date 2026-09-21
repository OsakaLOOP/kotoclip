use crate::model::{Register, RegisterRouting};
use serde::{Deserialize, Serialize};

pub const VERSION: &str = "kotoclip.register-routing.v2";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegisterPolicy { #[default] Auto, Cwj, Csj }

impl From<Register> for RegisterPolicy {
    fn from(value: Register) -> Self { match value { Register::Cwj => Self::Cwj, Register::Csj => Self::Csj } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisterRun {
    pub char_range: [usize; 2],
    pub selected: Register,
    pub reason: String,
}

pub fn select_register(text: &str, requested: Register) -> RegisterRouting {
    route_text(text, requested.into())
}

pub fn route_text(text: &str, policy: RegisterPolicy) -> RegisterRouting {
    let chars: Vec<_> = text.chars().collect();
    let mut runs = Vec::new();
    if policy != RegisterPolicy::Auto {
        let selected = if policy == RegisterPolicy::Csj { Register::Csj } else { Register::Cwj };
        if !chars.is_empty() { runs.push(RegisterRun { char_range: [0, chars.len()], selected, reason: "explicit_selection".into() }); }
    } else {
        let mut cursor = 0;
        let mut opening = 0;
        let mut quotes = Vec::new();
        for (index, &character) in chars.iter().enumerate() {
            match character {
                '「' | '『' => {
                    if quotes.is_empty() { opening = index; }
                    quotes.push(if character == '「' { '」' } else { '』' });
                }
                '」' | '』' if quotes.last() == Some(&character) => {
                    quotes.pop();
                    if quotes.is_empty() && index - opening > 11 {
                        push(&mut runs, [cursor, opening], Register::Cwj, "narration");
                        push(&mut runs, [opening, index + 1], Register::Csj, "quoted_passage");
                        cursor = index + 1;
                    }
                }
                _ => {},
            }
        }
        if !quotes.is_empty() && chars.len() - opening > 11 {
            push(&mut runs, [cursor, opening], Register::Cwj, "narration");
            push(&mut runs, [opening, chars.len()], Register::Csj, "open_quotation");
        } else {
            push(&mut runs, [cursor, chars.len()], Register::Cwj, "narration");
        }
    }
    let selected = runs.first().map(|run| run.selected).filter(|selected| runs.iter().all(|run| run.selected == *selected));
    RegisterRouting { version: VERSION.into(), requested: policy, selected, runs }
}

fn push(runs: &mut Vec<RegisterRun>, char_range: [usize; 2], selected: Register, reason: &str) {
    if char_range[0] < char_range[1] { runs.push(RegisterRun { char_range, selected, reason: reason.into() }); }
}

/// 从全文策略提取窗口，保留窗口起点之前的引号上下文。
pub fn slice(routing: &RegisterRouting, range: [usize; 2]) -> RegisterRouting {
    let runs: Vec<_> = routing.runs.iter().filter_map(|run| {
        let start = range[0].max(run.char_range[0]);
        let end = range[1].min(run.char_range[1]);
        (start < end).then(|| RegisterRun { char_range: [start - range[0], end - range[0]], selected: run.selected, reason: run.reason.clone() })
    }).collect();
    let selected = runs.first().map(|run| run.selected).filter(|selected| runs.iter().all(|run| run.selected == *selected));
    RegisterRouting { version: routing.version.clone(), requested: routing.requested, selected, runs }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrative_and_nested_quotation_have_separate_sources() {
        let text = "彼は「今日は『本当に』よい天気ですね」と話した。";
        let routing = route_text(text, RegisterPolicy::Auto);
        assert_eq!(routing.runs.iter().map(|r| r.selected).collect::<Vec<_>>(), vec![Register::Cwj, Register::Csj, Register::Cwj]);
        assert_eq!(routing.selected, None);
        assert_eq!(routing.runs[0].char_range, [0, 2]);
        assert_eq!(routing.runs.last().unwrap().char_range[1], text.chars().count());
        assert!(routing.runs.windows(2).all(|p| p[0].char_range[1] == p[1].char_range[0]));
        assert_eq!(slice(&routing, [5, 12]).selected, Some(Register::Csj));
    }

    #[test]
    fn explicit_selection_and_short_quotes_keep_requested_dictionary() {
        assert_eq!(select_register("「これはとても長い会話文を含んでいます」", Register::Cwj).selected, Some(Register::Cwj));
        assert_eq!(route_text("『警察署』へ向かう。", RegisterPolicy::Auto).selected, Some(Register::Cwj));
        assert_eq!(route_text("冒頭「これは閉じていない会話の一部", RegisterPolicy::Auto).runs[1].reason, "open_quotation");
    }
}
