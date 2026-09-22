//! 活用与构词共用的 GiNZA 来源引用及局部关系查询。
use crate::{external::{Endpoint, NodeKind, SourceArtifact, SourceNode}, model::MorphemeToken};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceEvidence {
    pub provider: String,
    pub node_id: Option<String>,
    pub relation_id: Option<String>,
    pub reason: String,
}

pub struct Context<'a> { pub ginza: Option<&'a SourceArtifact> }

impl<'a> Context<'a> {
    pub fn new(sources: &'a [SourceArtifact]) -> Self {
        Self { ginza: sources.iter().find(|s| s.provider.id == "ginza") }
    }

    pub fn tokens(&self, range: [usize; 2]) -> Vec<&'a SourceNode> {
        self.ginza.into_iter().flat_map(|s| &s.nodes).filter(|n| n.kind == NodeKind::Token
            && n.text_ranges.len() == 1 && n.text_ranges[0][0] < range[1] && range[0] < n.text_ranges[0][1]).collect()
    }

    pub fn evidence(&self, range: [usize; 2]) -> Vec<SourceEvidence> {
        self.tokens(range).iter().map(|n| SourceEvidence { provider: "ginza".into(), node_id: Some(n.id.clone()),
            relation_id: None, reason: "formal_token".into() }).collect()
    }

    pub fn relation(&self, left: [usize; 2], right: [usize; 2], labels: &[&str]) -> Option<SourceEvidence> {
        let source = self.ginza?;
        let a = self.tokens(left); let b = self.tokens(right);
        source.relations.iter().find_map(|r| {
            if !labels.contains(&r.label.as_str()) { return None; }
            let (Endpoint::Node { id: from }, Endpoint::Node { id: to }) = (&r.source, &r.target) else { return None; };
            let linked = (a.iter().any(|n| &n.id == from) && b.iter().any(|n| &n.id == to))
                || (b.iter().any(|n| &n.id == from) && a.iter().any(|n| &n.id == to));
            linked.then(|| SourceEvidence { provider: "ginza".into(), node_id: None,
                relation_id: Some(r.id.clone()), reason: r.label.clone() })
        })
    }

    pub fn same_word(&self, left: [usize; 2], right: [usize; 2]) -> bool {
        self.tokens(left).iter().any(|n| n.text_ranges[0][0] <= left[0] && right[1] <= n.text_ranges[0][1])
    }

    pub fn same_bunsetsu(&self, left: [usize; 2], right: [usize; 2]) -> bool {
        self.ginza.into_iter().flat_map(|s| &s.nodes).any(|n| n.kind == NodeKind::Bunsetsu
            && n.text_ranges.len() == 1 && n.text_ranges[0][0] <= left[0] && right[1] <= n.text_ranges[0][1])
    }

    pub fn auxiliary(&self, range: [usize; 2]) -> bool {
        self.tokens(range).iter().any(|n| n.features["pos"].as_str() == Some("AUX"))
    }

    pub fn tag(&self, range: [usize; 2]) -> Option<&'a str> {
        self.tokens(range).into_iter().find(|n| n.text_ranges[0] == range).and_then(|n| n.features["tag"].as_str())
    }

    pub fn inflection(&self, range: [usize; 2]) -> Option<&'a str> {
        self.tokens(range).into_iter().find(|n| n.text_ranges[0] == range).and_then(|n| n.features["morph"]["Inflection"].as_str())
    }

    pub fn functional_position(&self, range: [usize; 2]) -> bool {
        self.tokens(range).iter().any(|n| n.features["bunsetu_position"].as_str() == Some("SYN_HEAD")
            || n.features["bunsetu_position"].as_str() == Some("FUNC"))
    }

    /// 来源词界与统一成员都完整时，正式词元可以确认一个复合词核心。
    pub fn core_members(&self, morphemes: &[MorphemeToken], start: usize) -> Vec<usize> {
        let range = morphemes[start].char_range;
        for node in self.tokens(range) {
            let [a, b] = node.text_ranges[0];
            if a != range[0] { continue; }
            let members: Vec<_> = (start..morphemes.len()).take_while(|&i| morphemes[i].char_range[0] < b).collect();
            if members.len() < 2 || morphemes[*members.last().unwrap()].char_range[1] != b { continue; }
            if members.windows(2).any(|p| morphemes[p[0]].char_range[1] != morphemes[p[1]].char_range[0]) { continue; }
            if members.iter().all(|&i| matches!(morphemes[i].pos[0].as_deref(), Some("名詞" | "動詞" | "形容詞" | "形状詞" | "接頭辞" | "接尾辞"))) {
                return members;
            }
        }
        vec![start]
    }
}
