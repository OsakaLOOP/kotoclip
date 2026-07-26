use crate::model::{ExecutionPlan, InfluenceScope, PlanDisposition, SemanticDelta};

pub fn containment_delta() -> SemanticDelta {
    SemanticDelta {
        change_id: "lexical.word_formation_overlap.proper_containment".to_string(),
        owner: "pipeline.lexical".to_string(),
        kind: "relation_predicate".to_string(),
        before_hash: "overlap-and-unequal".to_string(),
        after_hash: "crossing-overlap".to_string(),
        scope: InfluenceScope::Paragraph,
        old_selector: "accepted_formation && raw_lexical && non_equal_overlap".to_string(),
        new_selector: "accepted_formation && raw_lexical && crossing_overlap".to_string(),
        observation_set: vec![
            "lexical_unit".to_string(),
            "bunsetsu".to_string(),
            "ui_projection".to_string(),
            "dictionary_request".to_string(),
        ],
        soundness: "exact_boolean_difference".to_string(),
        reads_absence: false,
        unbounded_context: false,
    }
}

pub fn plan(deltas: Vec<SemanticDelta>, classified_inputs: bool) -> ExecutionPlan {
    if !classified_inputs {
        return ExecutionPlan {
            disposition: PlanDisposition::Blocked,
            scope: InfluenceScope::Corpus,
            deltas,
            reasons: vec!["unclassified_execution_input".to_string()],
            excluded_features: excluded_features(),
        };
    }
    let requires_full_domain = deltas.iter().any(|delta| {
        delta.reads_absence
            || delta.unbounded_context
            || delta.scope == InfluenceScope::Corpus
            || delta.soundness == "unknown"
    });
    let scope = deltas
        .iter()
        .map(|delta| delta.scope)
        .max()
        .unwrap_or(InfluenceScope::Corpus);
    ExecutionPlan {
        disposition: if requires_full_domain {
            PlanDisposition::FullDomain
        } else {
            PlanDisposition::Selective
        },
        scope,
        deltas,
        reasons: if requires_full_domain {
            vec!["absence_unbounded_or_unknown_semantics".to_string()]
        } else {
            vec!["all_selectors_have_no_false_negative_contract".to_string()]
        },
        excluded_features: excluded_features(),
    }
}

fn excluded_features() -> Vec<String> {
    vec![
        "n_best".to_string(),
        "user_profile_scoring".to_string(),
        "user_segmentation_choices".to_string(),
        "custom_profile_expressions".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absence_predicate_falls_back_to_full_domain() {
        let mut delta = containment_delta();
        delta.reads_absence = true;
        assert_eq!(
            plan(vec![delta], true).disposition,
            PlanDisposition::FullDomain
        );
    }

    #[test]
    fn unclassified_input_blocks_audit() {
        assert_eq!(
            plan(vec![containment_delta()], false).disposition,
            PlanDisposition::Blocked
        );
    }
}
