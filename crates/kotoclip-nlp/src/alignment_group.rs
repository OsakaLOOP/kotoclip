//! 以字符交集构建二部图连通分量，保存多对多对应与两侧未覆盖范围。
use crate::{external::{merge_ranges, NodeKind, Range, SourceArtifact}, model::MorphemeToken, syntax::SyntaxArtifact};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlignmentMember {
    pub id: String,
    pub text_ranges: Vec<Range>,
    pub source_ranges: Option<Vec<Range>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Intersection {
    pub provider_token_id: String,
    pub morpheme_id: String,
    pub char_range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroupStatus { Complete, Partial, Unmatched }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlignmentGroup {
    pub id: String,
    pub cardinality: String,
    pub status: GroupStatus,
    pub provider_tokens: Vec<AlignmentMember>,
    pub morpheme_ids: Vec<String>,
    pub char_range: Range,
    pub intersections: Vec<Intersection>,
    pub provider_gaps: Vec<Range>,
    pub morpheme_gaps: Vec<Range>,
    pub reason: String,
}

fn root(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index { parents[index] = root(parents, parents[index]); }
    parents[index]
}

fn gaps(range: Range, covered: Vec<Range>) -> Vec<Range> {
    let mut cursor = range[0];
    let mut result = Vec::new();
    for [a, b] in merge_ranges(covered) {
        if cursor < a { result.push([cursor, a]); }
        cursor = cursor.max(b);
    }
    if cursor < range[1] { result.push([cursor, range[1]]); }
    result
}

pub fn from_source(source: &SourceArtifact, morphemes: &[MorphemeToken]) -> Vec<AlignmentGroup> {
    let members = source.nodes.iter().filter(|node| node.kind == NodeKind::Token).map(|node| AlignmentMember {
        id: node.id.clone(), text_ranges: node.text_ranges.clone(), source_ranges: Some(node.source_ranges.clone()),
    }).collect();
    collect(&source.provider.id, members, morphemes)
}

pub fn from_syntax(source: &SyntaxArtifact, morphemes: &[MorphemeToken]) -> Vec<AlignmentGroup> {
    let members = source.spans.iter().filter(|node| node.kind == "token").map(|node| AlignmentMember {
        id: node.id.clone(), text_ranges: vec![node.char_range], source_ranges: None,
    }).collect();
    collect(&source.provider.id, members, morphemes)
}

fn collect(provider: &str, members: Vec<AlignmentMember>, morphemes: &[MorphemeToken]) -> Vec<AlignmentGroup> {
    let count = members.len();
    let mut parents: Vec<usize> = (0..count + morphemes.len()).collect();
    let mut edges = Vec::new();
    for (index, member) in members.iter().enumerate() {
        for &[a, b] in &member.text_ranges {
            let first = morphemes.partition_point(|token| token.char_range[1] <= a);
            for (offset, token) in morphemes[first..].iter().enumerate().take_while(|(_, token)| token.char_range[0] < b) {
                let other = first + offset;
                let left = root(&mut parents, index);
                let right = root(&mut parents, count + other);
                parents[right] = left;
                edges.push((index, other, [a.max(token.char_range[0]), b.min(token.char_range[1])]));
            }
        }
    }
    let mut components: BTreeMap<usize, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for index in 0..parents.len() {
        let group = components.entry(root(&mut parents, index)).or_default();
        if index < count { group.0.push(index); } else { group.1.push(index - count); }
    }
    let mut result = Vec::new();
    for (parent, (left, right)) in components {
        let provider_ranges: Vec<_> = left.iter().flat_map(|&i| members[i].text_ranges.iter().copied()).collect();
        let morpheme_ranges: Vec<_> = right.iter().map(|&i| morphemes[i].char_range).collect();
        let all: Vec<_> = provider_ranges.iter().chain(&morpheme_ranges).copied().collect();
        let range = [all.iter().map(|r| r[0]).min().unwrap(), all.iter().map(|r| r[1]).max().unwrap()];
        let provider_gaps = gaps(range, provider_ranges);
        let morpheme_gaps = gaps(range, morpheme_ranges);
        let (status, reason) = if left.is_empty() || right.is_empty() {
            (GroupStatus::Unmatched, "one_sided_content")
        } else if provider_gaps == morpheme_gaps {
            (GroupStatus::Complete, if provider_gaps.is_empty() { "equal_coverage" } else { "equal_coverage_with_gaps" })
        } else { (GroupStatus::Partial, "different_coverage") };
        let cardinality = match (left.len(), right.len()) {
            (0, _) | (_, 0) => "unmatched", (1, 1) => "1:1", (1, _) => "1:n", (_, 1) => "n:1", _ => "n:m",
        };
        let intersections = edges.iter().filter(|&&(i, _, _)| root(&mut parents, i) == parent).map(|&(i, j, char_range)| Intersection {
            provider_token_id: members[i].id.clone(), morpheme_id: morphemes[j].id.clone(), char_range,
        }).collect();
        let member_ids = left.iter().map(|&i| members[i].id.as_str()).chain(right.iter().map(|&i| morphemes[i].id.as_str())).collect::<Vec<_>>().join(",");
        result.push(AlignmentGroup {
            id: format!("align:{provider}:{}:{}:{member_ids}", range[0], range[1]), cardinality: cardinality.into(), status,
            provider_tokens: left.iter().map(|&i| members[i].clone()).collect(),
            morpheme_ids: right.iter().map(|&i| morphemes[i].id.clone()).collect(), char_range: range,
            intersections, provider_gaps, morpheme_gaps, reason: reason.into(),
        });
    }
    result.sort_by(|a, b| a.char_range.cmp(&b.char_range).then(a.id.cmp(&b.id)));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn align(left: &[Range], right: &[Range]) -> Vec<AlignmentGroup> {
        let members = left.iter().enumerate().map(|(i, r)| AlignmentMember { id: format!("t{i}"), text_ranges: vec![*r], source_ranges: None }).collect();
        let tokens: Vec<_> = right.iter().enumerate().map(|(i, r)| MorphemeToken { id: format!("m{i}"), source_index: i, surface: String::new(), char_range: *r, pos: [None, None, None, None], lemma: None, reading: None, query_forms: Vec::new() }).collect();
        collect("fixture", members, &tokens)
    }

    #[test]
    fn groups_all_four_cardinalities() {
        for (left, right, cardinality) in [
            (vec![[0, 4]], vec![[0, 4]], "1:1"),
            (vec![[0, 4]], vec![[0, 2], [2, 4]], "1:n"),
            (vec![[0, 2], [2, 4]], vec![[0, 4]], "n:1"),
            (vec![[0, 1], [1, 4]], vec![[0, 2], [2, 4]], "n:m"),
        ] {
            let groups = align(&left, &right);
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].cardinality, cardinality);
            assert_eq!(groups[0].status, GroupStatus::Complete);
        }
    }

    #[test]
    fn separates_shared_boundaries_and_preserves_gap_and_partial_cut() {
        assert_eq!(align(&[[0, 2], [2, 4]], &[[0, 2], [2, 4]]).len(), 2);
        let groups = align(&[[0, 3]], &[[0, 1], [2, 3]]);
        assert_eq!(groups[0].status, GroupStatus::Partial);
        assert_eq!(groups[0].morpheme_gaps, vec![[1, 2]]);
        let groups = align(&[[1, 3]], &[[0, 2], [2, 4]]);
        assert_eq!(groups[0].provider_gaps, vec![[0, 1], [3, 4]]);
        assert_eq!(groups[0].intersections.len(), 2);
    }

    #[test]
    fn normalization_expansion_and_unmatched_content_remain_explicit() {
        let groups = align(&[[0, 1], [0, 1], [2, 3]], &[[0, 1], [4, 5]]);
        assert_eq!(groups[0].cardinality, "n:1");
        assert_eq!(groups[0].status, GroupStatus::Complete);
        assert_eq!(groups[1].status, GroupStatus::Unmatched);
        assert_eq!(groups[2].status, GroupStatus::Unmatched);
    }
}
