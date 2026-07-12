use super::morpheme::MorphemeCandidate;
use crate::models::{AnnotatedToken, Morpheme, SegmentationCandidate};

fn token_from_morphemes(morphemes: Vec<Morpheme>, source: &AnnotatedToken) -> AnnotatedToken {
    AnnotatedToken {
        bunsetsu: super::bunsetsu::build_bunsetsu(morphemes),
        novelty_score: source.novelty_score,
        is_selected: false,
        is_known: source.is_known,
        inference_reason: source.inference_reason.clone(),
        expressions: Vec::new(),
        display_class: source.display_class.clone(),
    }
}

/// 将真实 lattice 路径转换成 UI 可应用的 token 序列，并把局部字符范围
/// 平移回原文范围。这里不再合成任意边界候选。
pub fn from_lattice(
    source: &AnnotatedToken,
    paths: Vec<MorphemeCandidate>,
) -> Vec<SegmentationCandidate> {
    let best_cost = paths.first().map_or(0, |path| path.total_cost);
    let offset = source.bunsetsu.char_range.0;
    paths
        .into_iter()
        .enumerate()
        .map(|(vibrato_rank, mut path)| {
            for morpheme in &mut path.morphemes {
                morpheme.char_range.0 += offset;
                morpheme.char_range.1 += offset;
            }
            SegmentationCandidate {
                tokens: path
                    .morphemes
                    .into_iter()
                    .map(|morpheme| token_from_morphemes(vec![morpheme], source))
                    .collect(),
                total_cost: path.total_cost,
                relative_cost: path.total_cost.saturating_sub(best_cost),
                source: "vibrato_lattice".to_string(),
                vibrato_rank: vibrato_rank + 1,
                rank_score: i64::from(path.total_cost),
                dictionary_evidence: Vec::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Bunsetsu, HeadWord, PosTag};

    fn morpheme(surface: &str, start: usize) -> Morpheme {
        Morpheme {
            surface: surface.to_string(),
            pos: PosTag {
                major: "名詞".to_string(),
                sub1: "一般".to_string(),
                sub2: "*".to_string(),
                sub3: "*".to_string(),
            },
            base_form: surface.to_string(),
            reading: surface.to_string(),
            conjugation_type: "*".to_string(),
            conjugation_form: "*".to_string(),
            char_range: (start, start + 1),
        }
    }

    fn source_token() -> AnnotatedToken {
        let morphemes = vec![morpheme("警", 0), morpheme("察", 1), morpheme("署", 2)];
        AnnotatedToken {
            bunsetsu: Bunsetsu {
                morphemes,
                surface: "警察署".to_string(),
                head_word: HeadWord {
                    surface: "警察署".to_string(),
                    base_form: "警察署".to_string(),
                    reading: "ケイサツショ".to_string(),
                    pos: PosTag {
                        major: "名詞".to_string(),
                        sub1: "一般".to_string(),
                        sub2: "*".to_string(),
                        sub3: "*".to_string(),
                    },
                },
                grammar_tags: Vec::new(),
                char_range: (0, 3),
            },
            novelty_score: 0.8,
            is_selected: true,
            is_known: false,
            inference_reason: None,
            expressions: Vec::new(),
            display_class: "content".to_string(),
        }
    }

    #[test]
    fn converts_lattice_candidates_without_inventing_boundaries() {
        let source = source_token();
        let candidates = from_lattice(
            &source,
            vec![MorphemeCandidate {
                morphemes: source.bunsetsu.morphemes.clone(),
                total_cost: 42,
            }],
        );
        let surfaces: Vec<Vec<&str>> = candidates
            .iter()
            .map(|candidate| {
                candidate
                    .tokens
                    .iter()
                    .map(|token| token.bunsetsu.surface.as_str())
                    .collect()
            })
            .collect();

        assert_eq!(surfaces, vec![vec!["警", "察", "署"]]);
        assert_eq!(candidates[0].total_cost, 42);
        assert_eq!(candidates[0].source, "vibrato_lattice");
    }
}
