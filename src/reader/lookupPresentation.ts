import type { DictionaryLookup } from "../types";
import type { QueryOutput, UnifiedDocument } from "../types/nlp";

export interface ReaderChainSummary {
  id: string;
  role: string;
  surface: string;
  lemma: string;
  lookupForm: string;
  conjugation: string;
  formName: string;
  connections: string[];
  members: { surface: string; role: string }[];
  steps: { label: string; description: string; normalizedForm: string | null }[];
}

export interface ReaderMorphologyDetail {
  id: string;
  title: string;
  surface: string;
  description: string;
  normalizedForm: string | null;
  candidates: string[];
}

export interface ReaderMorphologyHit {
  range: [number, number];
  targetId: string;
  detail: ReaderMorphologyDetail;
}

export interface ReaderCapsuleRange { id: string; range: [number, number] }

export function readerCapsuleRanges(document: UnifiedDocument): ReaderCapsuleRange[] {
  const characters = Array.from(document.text);
  const blocked = document.morphemes.filter((token) =>
    ["記号", "補助記号", "空白"].includes(token.pos[0] ?? "") || /\s/u.test(token.surface));
  const valid = ([start, end]: [number, number]) => start < end && start >= 0 && end <= characters.length
    && !characters.slice(start, end).some((character) => /\s/u.test(character))
    && !blocked.some((token) => token.char_range[0] < end && token.char_range[1] > start);
  const chains = document.morphology.chains;
  const lexical = chains.filter((chain) => chain.role === "lexical" && !chain.parent_chain_id)
    .map((chain) => {
      const descendants = new Set([chain.chain_id]);
      let end = chain.char_range[1];
      for (let changed = true; changed;) {
        changed = false;
        for (const child of chains) {
          if (!child.parent_chain_id || !descendants.has(child.parent_chain_id) || descendants.has(child.chain_id)) continue;
          descendants.add(child.chain_id);
          if (child.char_range[0] <= end && valid([chain.char_range[0], child.char_range[1]])) {
            end = Math.max(end, child.char_range[1]);
          }
          changed = true;
        }
      }
      return { id: chain.chain_id, range: [chain.char_range[0], end] as [number, number] };
    }).filter((capsule) => valid(capsule.range));
  const formations = document.formation.nodes.filter((node) => node.status === "observed" && node.word)
    .map((node) => {
      const attached = lexical.filter((chain) => chain.range[0] >= node.char_range[0] && chain.range[0] < node.char_range[1])
        .sort((left, right) => left.range[0] - right.range[0]);
      const end = attached.reduce((current, chain) => chain.range[0] <= current
        && valid([node.char_range[0], Math.max(current, chain.range[1])]) ? Math.max(current, chain.range[1]) : current, node.char_range[1]);
      return { id: node.id, range: [node.char_range[0], end] as [number, number] };
    }).filter((capsule) => valid(capsule.range));
  const result: ReaderCapsuleRange[] = [];
  const byRange = (left: ReaderCapsuleRange, right: ReaderCapsuleRange) =>
    left.range[0] - right.range[0] || right.range[1] - left.range[1];
  for (const capsule of [...formations.sort(byRange), ...lexical.sort(byRange)]) {
    if (!result.some((previous) => capsule.range[0] < previous.range[1] && capsule.range[1] > previous.range[0])) result.push(capsule);
  }
  return result.sort(byRange);
}

const formLabels: Record<string, string> = {
  stem: "词干", irrealis: "未然形", continuative: "连用形", terminal: "终止形",
  attributive: "连体形", conditional: "假定形", imperative: "命令形",
  volitional: "意向形", te: "て形",
};

function grammaticalLabel(value: string) {
  return ({ "丁寧": "礼貌", "受身等候选": "被动／可能等", "接续": "接续" } as Record<string, string>)[value] ?? value;
}

function conjugationLabel(type = "", form = "", category = "", stateForm = "") {
  const family = type.startsWith("五段") ? "五段活用" : type.startsWith("一段") ? "一段活用"
    : type.startsWith("サ行変格") ? "サ变活用" : type.startsWith("カ行変格") ? "カ变活用" : "";
  const formName = Object.entries({ "未然形": "未然形", "連用形": "连用形", "終止形": "终止形", "連体形": "连体形", "仮定形": "假定形", "已然形": "已然形", "命令形": "命令形", "意志推量形": "意向形" })
    .find(([prefix]) => form.startsWith(prefix))?.[1] ?? formLabels[stateForm] ?? "";
  return [family || (category === "形容詞" ? "形容词" : ""), formName].filter(Boolean).join(" · ");
}

export function readerChains(
  target: { lexical_core_ids: string[]; source_formation_ids: string[]; morpheme_ids: string[] },
  document: UnifiedDocument | null | undefined,
): ReaderChainSummary[] {
  if (!document) return [];
  const chainIds = new Set(target.lexical_core_ids);
  for (const node of document.formation.nodes) {
    if (target.source_formation_ids.includes(node.id)) {
      for (const id of node.word?.chain_ids ?? []) chainIds.add(id);
    }
  }
  let chains = document.morphology.chains.filter((chain) => chainIds.has(chain.chain_id));
  if (!chains.length) {
    const members = new Set(target.morpheme_ids);
    chains = document.morphology.chains.filter((chain) =>
      chain.role === "lexical" && chain.core_morpheme_indices.some((index) => members.has(document.morphemes[index]?.id)));
  }
  for (const chain of chains) chainIds.add(chain.chain_id);
  for (let index = 0; index < chains.length; index++) {
    chains.push(...document.morphology.chains.filter((chain) => chain.parent_chain_id === chains[index].chain_id && !chainIds.has(chain.chain_id)));
    for (const chain of chains) chainIds.add(chain.chain_id);
  }
  return chains.map((chain) => ({
    id: chain.chain_id,
    role: chain.role === "functional" ? "补助成分" : "词汇核心",
    surface: chain.surface_form,
    lemma: chain.lemma_form || chain.dictionary_form || chain.base_lexeme,
    lookupForm: chain.lookup_form,
    conjugation: conjugationLabel(chain.final_state?.conjugation_type, chain.final_state?.conjugation_form, chain.final_state?.category, chain.final_state?.form),
    formName: [...new Set(chain.operators.filter((operator) => !["conjugation", "initial_alternation", "final_alternation"].includes(operator.kind))
      .map((operator) => grammaticalLabel(operator.label)).filter(Boolean))].join(" · "),
    connections: chain.connection_forms.filter((value) => value && value !== "*"),
    members: chain.morpheme_indices.flatMap((index) => {
      const morpheme = document.morphemes[index];
      return morpheme ? [{ surface: morpheme.surface, role: chain.core_morpheme_indices.includes(index) ? "核心" : morpheme.pos[0] || "连接" }] : [];
    }),
    steps: chain.operators.map((operator) => ({
      label: grammaticalLabel(operator.label),
      description: operator.description || "",
      normalizedForm: operator.normalized_form,
    })),
  }));
}

export function readerMorphologyHits(
  targets: { id: string; char_range: [number, number]; lexical_core_ids: string[]; source_formation_ids: string[] }[],
  document: UnifiedDocument | null | undefined,
): ReaderMorphologyHit[] {
  if (!document) return [];
  const source = document;
  const chains = new Map(document.morphology.chains.map((chain) => [chain.chain_id, chain]));
  const characters = Array.from(document.text);
  function targetForChain(chainId: string) {
    let root = chains.get(chainId);
    if (!root) return undefined;
    while (root.parent_chain_id && chains.has(root.parent_chain_id)) root = chains.get(root.parent_chain_id)!;
    return targets.find((item) => item.lexical_core_ids.includes(root.chain_id))
      ?? targets.find((item) => item.source_formation_ids.some((id) => source.formation.nodes.find((node) => node.id === id)?.word?.chain_ids.includes(root.chain_id)))
      ?? targets.find((item) => item.char_range[0] <= root.anchor_range[0] && root.anchor_range[1] <= item.char_range[1]);
  }
  const occurrences = document.morphology.occurrences.flatMap((occurrence) => {
    const target = targetForChain(occurrence.chain_id);
    if (!target) return [];
    const operators = occurrence.operator_ids.flatMap((id) => document.morphology.chains.flatMap((chain) => chain.operators.filter((operator) => operator.operator_id === id)));
    const operator = operators.find((item) => item.kind === occurrence.kind) ?? operators[operators.length - 1];
    if (!operator) return [];
    const detail: ReaderMorphologyDetail = {
      id: occurrence.id,
      title: grammaticalLabel(operator.label) || "活用",
      surface: characters.slice(occurrence.char_range[0], occurrence.char_range[1]).join(""),
      description: operator.description || "",
      normalizedForm: operator.normalized_form,
      candidates: occurrence.candidates,
    };
    return occurrence.hit_ranges.map((range) => ({ range, targetId: target.id, detail }));
  });
  const conjugations = document.morphology.chains.flatMap((chain) => {
    const target = targetForChain(chain.chain_id);
    if (!target || chain.surface_form === (chain.lemma_form || chain.dictionary_form)) return [];
    return chain.operators.filter((operator) => operator.kind === "conjugation" && operator.char_range[0] < operator.char_range[1])
      .map((operator) => ({
        range: operator.char_range,
        targetId: target.id,
        detail: {
          id: operator.operator_id,
          title: conjugationLabel(chain.final_state.conjugation_type, chain.final_state.conjugation_form, chain.final_state.category, chain.final_state.form) || "活用",
          surface: characters.slice(operator.char_range[0], operator.char_range[1]).join(""),
          description: `${chain.lemma_form || chain.dictionary_form} → ${chain.surface_form}`,
          normalizedForm: null,
          candidates: [],
        },
      }));
  });
  return [...occurrences, ...conjugations];
}

export function dictionaryLookupFromSearch(output: QueryOutput, word: string, formId = output.selected_form_id): DictionaryLookup {
  const group = output.groups.find((item) => item.form_id === formId) ?? output.groups[0];
  return {
    query: word,
    observed_form: word,
    reading: output.reading ?? null,
    mode: output.mode || "search",
    forms: output.forms,
    selected_form_id: formId ?? output.forms[0]?.form_id ?? null,
    dictionary_names: output.dictionary_names,
    entries: group?.entries ?? [],
  };
}
