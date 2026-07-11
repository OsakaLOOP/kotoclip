use crate::models::{AnnotatedToken, Morpheme, SegmentationCandidate};
use std::collections::HashSet;

fn token_from_morphemes(morphemes: Vec<Morpheme>, source: &AnnotatedToken) -> AnnotatedToken {
    AnnotatedToken {
        bunsetsu: super::bunsetsu::build_bunsetsu(morphemes),
        novelty_score: source.novelty_score,
        is_selected: false,
        is_known: source.is_known,
        inference_reason: source.inference_reason.clone(),
        expressions: Vec::new(),
    }
}

fn candidate_from_ranges(
    source: &AnnotatedToken,
    ranges: &[(usize, usize)],
) -> SegmentationCandidate {
    SegmentationCandidate {
        tokens: ranges
            .iter()
            .map(|&(start, end)| {
                token_from_morphemes(source.bunsetsu.morphemes[start..end].to_vec(), source)
            })
            .collect(),
    }
}

fn candidate_key(candidate: &SegmentationCandidate) -> String {
    candidate
        .tokens
        .iter()
        .map(|token| token.bunsetsu.surface.as_str())
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

pub fn split_token(source: &AnnotatedToken) -> Vec<AnnotatedToken> {
    source
        .bunsetsu
        .morphemes
        .iter()
        .cloned()
        .map(|morpheme| token_from_morphemes(vec![morpheme], source))
        .collect()
}

/// Generate deterministic alternatives from real morpheme boundaries.
/// The all-morpheme split is first, followed by every binary split ordered
/// from left to right. Duplicate surface segmentations are removed.
pub fn get_candidates(source: &AnnotatedToken, top_n: usize) -> Vec<SegmentationCandidate> {
    let count = source.bunsetsu.morphemes.len();
    if count < 2 || top_n == 0 {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let all_ranges: Vec<(usize, usize)> = (0..count).map(|index| (index, index + 1)).collect();
    let all_split = candidate_from_ranges(source, &all_ranges);
    seen.insert(candidate_key(&all_split));
    candidates.push(all_split);

    for boundary in 1..count {
        let candidate = candidate_from_ranges(source, &[(0, boundary), (boundary, count)]);
        if seen.insert(candidate_key(&candidate)) {
            candidates.push(candidate);
        }
        if candidates.len() >= top_n {
            break;
        }
    }

    candidates.truncate(top_n);
    candidates
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
        }
    }

    #[test]
    fn generates_real_boundary_candidates() {
        let candidates = get_candidates(&source_token(), 3);
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

        assert_eq!(surfaces[0], vec!["警", "察", "署"]);
        assert_eq!(surfaces[1], vec!["警", "察署"]);
        assert_eq!(surfaces[2], vec!["警察", "署"]);
    }
}
