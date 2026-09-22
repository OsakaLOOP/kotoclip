//! 词法连接状态机。GiNZA 参与复合核心、功能所有权和来源冲突判定。
use crate::{external::SourceArtifact, linguistic_context::Context, model::{MorphemeToken, ProviderToken, QueryForm}, morphology::*};

fn field(t: &ProviderToken, n: usize) -> &str { t.fields.get(n).and_then(|f| f.value.as_deref()).unwrap_or("") }

pub(crate) fn state(t: &ProviderToken) -> MorphologyState {
    let raw = field(t, 5);
    let form = if raw.starts_with("未然形") { ConnectionForm::Irrealis }
        else if raw.starts_with("連用形") { ConnectionForm::Continuative }
        else if raw.starts_with("終止形") { ConnectionForm::Terminal }
        else if raw.starts_with("連体形") { ConnectionForm::Attributive }
        else if raw.starts_with("仮定形") || raw.starts_with("已然形") { ConnectionForm::Conditional }
        else if raw.starts_with("命令形") { ConnectionForm::Imperative }
        else if raw.starts_with("意志推量形") { ConnectionForm::Volitional }
        else if raw.is_empty() || raw.starts_with("語幹") { ConnectionForm::Stem }
        else { ConnectionForm::Other };
    MorphologyState { category: field(t, 0).into(), form, conjugation_type: field(t, 4).into(), conjugation_form: raw.into() }
}

fn identity(t: &ProviderToken) -> &str {
    let base = field(t, 10); if base.is_empty() { field(t, 7) } else { base }
}

fn semantic(t: &ProviderToken) -> Vec<String> {
    let mut kinds = features(t);
    if state(t).form == ConnectionForm::Imperative { kinds.push("imperative".into()); }
    if field(t, 0) == "形容詞" && matches!(identity(t), "ない" | "無い") { kinds.push("negative".into()); }
    if field(t,0)=="助動詞" || (field(t,0)=="形状詞" && field(t,1).contains("助動詞")) {
        let kind=match identity(t) {
            "そう" | "そうだ" => Some("sou_modality"), "よう" | "ようだ" => Some("you_modality"),
            "みたい" | "みたいだ" => Some("mitai_modality"), "らしい" => Some("rashii_modality"),
            "べし" | "べきだ" => Some("obligation"), "まい" => Some("negative_volitional"),
            "たがる" => Some("desire_outward"), _=>None,
        };
        if let Some(kind)=kind { kinds.push(kind.into()); }
    }
    if field(t,0)=="接尾辞" {
        let kind=match identity(t) { "やすい" | "易い"=>Some("ease"), "にくい" | "難い" | "づらい" | "辛い"=>Some("difficulty"), "さ"=>Some("nominalization"), _=>None };
        if let Some(kind)=kind { kinds.push(kind.into()); }
    }
    let mut seen=std::collections::BTreeSet::new();
    kinds.retain(|kind| seen.insert(kind.clone())); kinds
}

enum Transition { Core, Attach(Vec<String>), Support(&'static str), Stop }

fn transition(previous: &MorphologyChain, next: &ProviderToken, context: &Context<'_>, source: &[ProviderToken]) -> Transition {
    use ConnectionForm::*;
    let last = &source[previous.morpheme_range[1] - 1];
    let form = &previous.final_state.form;
    let pos = field(next, 0); let base = identity(next); let ctype = field(next, 4);
    let aux_relation = context.relation(previous.char_range, next.char_range, &["aux", "cop"]).is_some();
    let sahen = field(last, 0) == "名詞" && (field(last, 2).contains("サ変") || context.tag(last.char_range).is_some_and(|tag| tag.contains("サ変")))
        && pos == "動詞" && ctype.contains("サ行変格") && matches!(base, "する" | "為る");
    if sahen { return Transition::Core; }
    if pos=="接尾辞" && matches!(form, Continuative | Stem) && matches!(previous.final_state.category.as_str(),"動詞"|"形容詞"|"形状詞")
        && matches!(base,"やすい"|"易い"|"にくい"|"難い"|"づらい"|"辛い"|"さ") { return Transition::Core; }
    if *form==Continuative && pos=="動詞" && field(next,1).starts_with("非自立") {
        let kind=match base { "すぎる"|"過ぎる"=>Some("excessive"), "はじめる"|"始める"=>Some("inceptive"),
            "つづける"|"続ける"=>Some("continuative_aspect"), "おわる"|"終わる"=>Some("terminative"), _=>None };
        if let Some(kind)=kind { return Transition::Support(kind); }
    }
    if matches!(pos, "動詞" | "助動詞") && matches!(base, "てる" | "でる" | "ちゃう" | "じゃう" | "とく" | "どく")
        && matches!(form, Continuative | Te) {
        return Transition::Support(match base { "ちゃう" | "じゃう" => "contracted_te_shimau", "とく" | "どく" => "contracted_te_oku", _ => "contracted_te_iru" });
    }
    if field(last, 4) == "助動詞-ダ" && last.surface == "で" && matches!(base, "ある" | "有る") {
        return Transition::Attach(vec!["copula_aru".into()]);
    }
    if *form == Te && matches!(pos, "動詞" | "形容詞" | "助動詞") {
        let function = match base {
            "いる" | "居る" | "おる" | "居る-オル" => Some("te_iru"), "ある" | "有る" => Some("te_aru"),
            "しまう" | "仕舞う" => Some("te_shimau"), "おく" | "置く" => Some("te_oku"),
            "いく" | "行く" => Some("te_iku"), "くる" | "来る" => Some("te_kuru"),
            "みる" | "見る" => Some("te_miru"), "くださる" | "下さる" => Some("te_kudasaru"),
            "もらう" | "貰う" => Some("te_morau"), "あげる" | "上げる" => Some("te_ageru"),
            "くれる" | "呉れる" => Some("te_kureru"), "ほしい" | "欲しい" => Some("te_hoshii"), _ => None,
        };
        if let Some(function) = function {
            if field(next, 1).starts_with("非自立") || context.auxiliary(next.char_range) || context.functional_position(next.char_range) || aux_relation {
                return Transition::Support(function);
            }
        }
    }
    let nominal = matches!(previous.final_state.category.as_str(), "名詞" | "形状詞" | "代名詞");
    if pos=="形状詞" && field(next,1).contains("助動詞") && matches!(base,"そう"|"よう"|"みたい")
        && (matches!(form,Continuative|Terminal|Attributive) || (previous.final_state.category=="形容詞" && *form==Stem)) {
        return Transition::Attach(semantic(next));
    }
    let attach = if pos == "助動詞" {
        if matches!(base, "れる" | "られる" | "せる" | "させる" | "しめる") { *form == Irrealis }
        else if matches!(ctype, "助動詞-ナイ" | "助動詞-ヌ" | "助動詞-ズ") { *form == Irrealis }
        else if matches!(ctype, "助動詞-タ" | "助動詞-マス" | "助動詞-タイ") { *form == Continuative }
        else if ctype == "助動詞-ダ" { nominal }
        else if ctype == "助動詞-デス" { nominal || matches!(form, Terminal | Attributive) }
        else if base=="たがる" { *form==Continuative }
        else if base=="まい" { matches!(form,Terminal|Attributive|Irrealis|Continuative) }
        else if matches!(base, "そう" | "そうだ") { nominal || matches!(form, Continuative | Terminal | Attributive) }
        else if matches!(base, "よう" | "ようだ" | "らしい" | "べし" | "べきだ") { nominal || matches!(form, Terminal | Attributive) }
        else { aux_relation && !matches!(form, Stem | Te | Other) }
    } else if pos == "形容詞" && matches!(base, "ない" | "無い") {
        *form == Continuative && matches!(previous.final_state.category.as_str(), "形容詞" | "助動詞")
    } else { false };
    if attach { return Transition::Attach(semantic(next)); }
    if pos == "助詞" && field(next, 1) == "接続助詞" {
        let kind = match next.surface.as_str() {
            "て" | "で" if *form == Continuative => Some("te_connection"),
            "ば" if *form == Conditional => Some("ba_connection"),
            "ながら" if *form == Continuative => Some("nagara_connection"),
            "ど" | "ども" if *form == Conditional => Some("concessive_connection"), _ => None,
        };
        if let Some(kind) = kind { return Transition::Attach(vec![kind.into()]); }
    }
    if pos == "助詞" && field(next, 1) == "終助詞" && next.surface == "な" && matches!(form, Terminal | Attributive) {
        return Transition::Attach(vec!["prohibitive".into()]);
    }
    Transition::Stop
}

fn operator(chain: &MorphologyChain, atom: &MorphologyChain, kind: String) -> MorphologyOperator {
    MorphologyOperator { operator_id: format!("morphology:{}:{kind}", atom.anchor_morpheme),
        source_morpheme_range: atom.morpheme_range, char_range: atom.char_range,
        input_state: chain.surface_form.clone(), output_state: format!("{}{}", chain.surface_form, atom.surface_form),
        state_before: chain.final_state.clone(), state_after: atom.final_state.clone(),
        normalized_form: match kind.as_str() {
            "contracted_te_iru" => Some("ている".into()),
            "contracted_te_shimau" => Some("てしまう".into()),
            "contracted_te_oku" => Some("ておく".into()),
            _ => None,
        },
        concept_id: feature_concept(&kind).into(), confidence: 1.0, evidence: atom.evidence.clone(),
        candidates: if kind == "passive_potential" { vec!["受身".into(), "可能".into(), "尊敬".into(), "自発".into()] } else { Vec::new() },
        label: feature_label(&kind).into(), description: format!("{}：{}", feature_label(&kind), atom.surface_form), kind }
}

fn merge(chain: &mut MorphologyChain, atom: MorphologyChain, core: bool) {
    if core {
        let prefix = chain.surface_form.clone();
        let prefix_reading = chain.query_forms.iter().find(|f| f.kind == "observed").and_then(|f| f.reading.clone());
        let observed_reading = prefix_reading.as_ref().zip(atom.query_forms.iter().find(|f| f.kind == "observed").and_then(|f| f.reading.as_ref())).map(|(a,b)| format!("{a}{b}"));
        chain.dictionary_form = format!("{}{}", prefix, atom.dictionary_form);
        chain.lookup_form = chain.dictionary_form.clone(); chain.display_form = chain.dictionary_form.clone();
        chain.lemma_form = format!("{}{}", prefix, atom.lemma_form);
        chain.core_morpheme_indices.extend(&atom.core_morpheme_indices);
        chain.query_forms = atom.query_forms.iter().filter(|f| f.kind != "observed").map(|f| QueryForm {
            kind: f.kind.clone(), form: format!("{prefix}{}", f.form),
            reading: prefix_reading.as_ref().zip(f.reading.as_ref()).map(|(a,b)| format!("{a}{b}")),
            reading_field: Some("composed".into()),
        }).collect();
        chain.query_forms.insert(0, QueryForm { kind: "observed".into(), form: format!("{prefix}{}", atom.surface_form), reading_field: observed_reading.as_ref().map(|_| "composed_kana".into()), reading: observed_reading });
        if !chain.query_forms.iter().any(|f| f.form == chain.dictionary_form) {
            chain.query_forms.push(QueryForm { kind: "base".into(), form: chain.dictionary_form.clone(), reading: None, reading_field: None });
        }
    }
    chain.surface_form.push_str(&atom.surface_form);
    chain.morpheme_range[1] = atom.morpheme_range[1]; chain.char_range[1] = atom.char_range[1];
    chain.source_ranges.extend(atom.source_ranges); chain.morpheme_indices.extend(atom.morpheme_indices);
    chain.operators.extend(atom.operators.into_iter().filter(|o| o.kind != "conjugation")); chain.connection_forms.extend(atom.connection_forms);
    chain.evidence.extend(atom.evidence); chain.final_state = atom.final_state;
}

pub(crate) fn compose(atoms: Vec<MorphologyChain>, source: &[ProviderToken], morphemes: &[MorphemeToken], external: &[SourceArtifact]) -> MorphologyArtifact {
    let context = Context::new(external);
    let mut chains: Vec<MorphologyChain> = Vec::new();
    let mut diagnostics = Vec::new();
    for mut atom in atoms {
        let index = atom.anchor_morpheme;
        let current = &source[index];
        atom.source_evidence = context.evidence(atom.char_range);
        if atom.final_state.conjugation_form.is_empty() {
            if let Some((ctype, cform)) = context.inflection(atom.char_range).and_then(|f| f.split_once(';')) {
                let mut supplemented = current.clone();
                if supplemented.fields.len() >= 6 {
                    supplemented.fields[4].value = Some(ctype.into()); supplemented.fields[5].value = Some(cform.into());
                    atom.final_state = state(&supplemented);
                    atom.evidence.push("ginza:Inflection:missing_unidic_form".into());
                }
            }
        }
        let mut linked = false;
        if let Some(previous) = chains.last_mut().filter(|p| p.char_range[1] == atom.char_range[0] && p.morpheme_range[1] == index) {
            let first = previous.core_morpheme_indices[0];
            let members = context.core_members(morphemes, first);
            let step = if members.contains(&index) { Transition::Core } else { transition(previous, current, &context, source) };
            match step {
                Transition::Core => {
                    previous.evidence.push("connection:lexical_core".into());
                    let prior = &source[previous.morpheme_range[1]-1];
                    if field(prior,0)=="動詞" && field(current,0)=="名詞" {
                        previous.status="candidate".into();
                        diagnostics.push(format!("lexical_core_pos_conflict:{}:{index}",previous.chain_id));
                    }
                    previous.source_evidence.extend(atom.source_evidence.clone());
                    for kind in semantic(current) { previous.operators.push(operator(previous, &atom, kind)); }
                    merge(previous, atom, true); continue;
                }
                Transition::Attach(mut kinds) => {
                    let before = previous.final_state.clone();
                    let copula = kinds.iter().any(|k| k == "copula");
                    let te = kinds.iter().any(|k| k == "te_connection");
                    if te { atom.final_state.form = ConnectionForm::Te; }
                    if kinds.is_empty() { kinds.push("auxiliary".into()); }
                    for kind in kinds { previous.operators.push(operator(previous, &atom, kind)); }
                    if let Some(e) = context.relation(previous.char_range, atom.char_range, &["aux", "cop", "mark"]) { previous.source_evidence.push(e); }
                    if context.ginza.is_some() && !context.same_bunsetsu(previous.char_range, atom.char_range) {
                        diagnostics.push(format!("cross_bunsetsu:{}:{}", previous.chain_id, index));
                    }
                    if copula && before.category == "形状詞" { previous.display_form = format!("{}だ", previous.dictionary_form); }
                    previous.source_evidence.extend(atom.source_evidence.clone());
                    merge(previous, atom, false); continue;
                }
                Transition::Support(kind) => {
                    atom.role = MorphologyRole::Functional;
                    atom.parent_chain_id = Some(previous.chain_id.clone());
                    atom.operators.push(operator(previous, &atom, kind.into()));
                    let relation = context.relation(previous.char_range, atom.char_range, &["aux", "cop", "advcl", "xcomp"]);
                    let supported = relation.is_some() || context.same_word(previous.char_range, atom.char_range)
                        || context.same_bunsetsu(previous.char_range, atom.char_range);
                    atom.status = if supported { "resolved" } else { "candidate" }.into();
                    if let Some(e) = relation { atom.source_evidence.push(e); }
                    if let Some(source)=context.ginza {
                        for node in source.nodes.iter().filter(|n| n.kind==crate::external::NodeKind::Bunsetsu && n.text_ranges.len()==1
                            && n.text_ranges[0][0]<=previous.char_range[0] && atom.char_range[1]<=n.text_ranges[0][1]) {
                            atom.source_evidence.push(crate::linguistic_context::SourceEvidence {provider:"ginza".into(),node_id:Some(node.id.clone()),relation_id:None,reason:"support_bunsetsu".into()});
                        }
                    }
                    let expanded=match kind { "contracted_te_iru"=>Some(("いる","イル")),"contracted_te_shimau"=>Some(("しまう","シマウ")),"contracted_te_oku"=>Some(("おく","オク")),_=>None };
                    if let Some((base,reading))=expanded {
                        atom.display_form=atom.operators.last().and_then(|op| op.normalized_form.clone()).unwrap();
                        atom.dictionary_form=base.into(); atom.lookup_form=base.into();
                        atom.query_forms.push(QueryForm {kind:"expanded_auxiliary".into(),form:base.into(),reading:Some(reading.into()),reading_field:Some("contraction_rule".into())});
                    }
                    if !supported { diagnostics.push(format!("support_ownership_candidate:{}:{}", previous.chain_id, index)); }
                    linked = true;
                }
                Transition::Stop => {
                    if context.relation(previous.char_range, atom.char_range, &["aux", "cop"]).is_some() {
                        diagnostics.push(format!("source_connection_conflict:{}:{}", previous.chain_id, index));
                        atom.status = "candidate".into();
                    }
                }
            }
        }
        for kind in semantic(current) {
            let mut op = operator(&atom, &atom, kind);
            op.input_state = atom.dictionary_form.clone(); op.output_state = atom.surface_form.clone();
            atom.operators.push(op);
        }
        if !linked && field(current, 0) == "助動詞" {
            atom.status = "pending".into(); diagnostics.push(format!("unattached_auxiliary:{index}"));
        }
        chains.push(atom);
    }
    for chain in &mut chains {
        for source in external.iter().filter(|s| s.provider.id == "kwja") {
            for node in source.nodes.iter().filter(|n| matches!(n.kind, crate::external::NodeKind::Predicate | crate::external::NodeKind::BasicPhrase)) {
                if node.text_ranges.iter().any(|r| r[0] < chain.char_range[1] && chain.char_range[0] < r[1]) {
                    chain.source_evidence.push(crate::linguistic_context::SourceEvidence { provider: "kwja".into(), node_id: Some(node.id.clone()), relation_id: None, reason: "predicate_context".into() });
                }
            }
        }
    }
    let occurrences = chains.iter().flat_map(|chain| chain.operators.iter().filter(|op|
        !matches!(op.kind.as_str(), "conjugation" | "initial_alternation" | "final_alternation"))
        .map(|op| {
            let mut hit_ranges=vec![op.char_range];
            let mut members:Vec<_>=(op.source_morpheme_range[0]..op.source_morpheme_range[1]).collect();
            let mut operator_ids=vec![op.operator_id.clone()];
            let mut context_range=chain.char_range;
            if op.kind.starts_with("te_") && op.kind != "te_connection" && chain.parent_chain_id.is_some() {
                if let Some(parent)=chains.iter().find(|c| Some(&c.chain_id)==chain.parent_chain_id.as_ref()) {
                    if let Some(connector)=parent.operators.iter().find(|o| o.kind=="te_connection") {
                        hit_ranges.insert(0,connector.char_range);
                        members.splice(0..0,connector.source_morpheme_range[0]..connector.source_morpheme_range[1]);
                        operator_ids.insert(0,connector.operator_id.clone());
                    }
                }
            }
            let mut parent=chain.parent_chain_id.as_ref();
            while let Some(id)=parent {
                let Some(ancestor)=chains.iter().find(|c| &c.chain_id==id) else { break; };
                context_range[0]=ancestor.char_range[0]; parent=ancestor.parent_chain_id.as_ref();
            }
            MorphologyOccurrence { id: format!("occurrence:{}", op.operator_id), chain_id: chain.chain_id.clone(), operator_ids,
                kind: op.kind.clone(), char_range: [hit_ranges[0][0],op.char_range[1]], context_range, hit_ranges,
                morpheme_indices: members, candidates: op.candidates.clone(), status: chain.status.clone(), source_evidence: chain.source_evidence.clone() }
        })).collect();
    MorphologyArtifact { schema: SCHEMA.into(), chains, occurrences, diagnostics }
}
