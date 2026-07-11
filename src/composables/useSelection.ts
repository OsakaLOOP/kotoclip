import { ref, Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Paragraph } from "./useTokenization";
import { ExportEntry, DictEntry } from "../types";

export function useSelection(paragraphs: Ref<Paragraph[]>) {
  // 当前被用户标记选中的 tokens (用于导出)
  // 结构: Array of { paragraphId, tokenIndex }
  const selectedKeys = ref<{ paragraphId: number; tokenIndex: number }[]>([]);

  /**
   * 切换单个胶囊的选中状态
   */
  function toggleSelect(paragraphId: number, tokenIndex: number) {
    const p = paragraphs.value.find((para) => para.id === paragraphId);
    if (!p) return;
    
    const token = p.tokens[tokenIndex];
    if (!token) return;

    token.is_selected = !token.is_selected;

    if (token.is_selected) {
      selectedKeys.value.push({ paragraphId, tokenIndex });
    } else {
      selectedKeys.value = selectedKeys.value.filter(
        (key) => !(key.paragraphId === paragraphId && key.tokenIndex === tokenIndex)
      );
    }
  }

  /**
   * 标记单词为已知 (脱去胶囊，更新本地 SQLite 曝光库与汉字表)
   */
  async function markAsKnown(paragraphId: number, tokenIndex: number) {
    const p = paragraphs.value.find((para) => para.id === paragraphId);
    if (!p) return;
    
    const token = p.tokens[tokenIndex];
    if (!token) return;

    const baseForm = token.bunsetsu.head_word.base_form;
    const reading = token.bunsetsu.head_word.reading;

    try {
      // 调用 IPC 命令
      await invoke("mark_known", { baseForm, reading });
      
      // 更新前端对应的所有相同 base_form 的词，全部变为已知状态以获得顺畅的阅读体验
      updateLocalTokensKnownStatus(baseForm, true);
    } catch (err) {
      console.error("Mark Known Error:", err);
    }
  }

  /**
   * 标记单词为未知
   */
  async function markAsUnknown(paragraphId: number, tokenIndex: number) {
    const p = paragraphs.value.find((para) => para.id === paragraphId);
    if (!p) return;

    const token = p.tokens[tokenIndex];
    if (!token) return;

    const baseForm = token.bunsetsu.head_word.base_form;
    const reading = token.bunsetsu.head_word.reading;

    try {
      await invoke("mark_unknown", { baseForm, reading });
      
      // 重新让这些词显现为生词胶囊
      updateLocalTokensKnownStatus(baseForm, false);
    } catch (err) {
      console.error("Mark Unknown Error:", err);
    }
  }

  /**
   * 更新前端渲染树中，具有相同原形的全部词的已知状态
   */
  function updateLocalTokensKnownStatus(baseForm: string, isKnown: boolean) {
    for (const para of paragraphs.value) {
      for (const token of para.tokens) {
        if (token.bunsetsu.head_word.base_form === baseForm) {
          token.is_known = isKnown;
          if (isKnown) {
            token.is_selected = false;
            token.novelty_score = 0.0;
          } else {
            token.novelty_score = 1.0; // 还原为生词权重
          }
        }
      }
    }
    // 清理已被设为已知的已选 Keys
    if (isKnown) {
      selectedKeys.value = selectedKeys.value.filter(({ paragraphId, tokenIndex }) => {
        const p = paragraphs.value.find((para) => para.id === paragraphId);
        return p?.tokens[tokenIndex] ? !p.tokens[tokenIndex].is_known : true;
      });
    }
  }

  /**
   * 导出所有选中的词为结构化 Anki 格式 JSON
   * 提取选中词的整句上下文并标出高亮区间
   */
  const notes = ref<Record<string, string>>({});
  function updateNote(paragraphId: number, tokenIndex: number, note: string) {
    notes.value[`${paragraphId}-${tokenIndex}`] = note;
  }

  async function exportSelected(sourceText: string, lookupFn: (word: string, reading?: string) => Promise<DictEntry[]>) {
    const exportEntries: ExportEntry[] = [];

    for (const key of selectedKeys.value) {
      const p = paragraphs.value.find((para) => para.id === key.paragraphId);
      if (!p) continue;
      
      const token = p.tokens[key.tokenIndex];
      if (!token) continue;

      // 拼凑整句上下文：这里通过把段落里的所有 tokens 拼合，并找到该 token 的相对范围来实现
      let contextSentence = "";
      let highlightStart = 0;
      let highlightEnd = 0;

      for (let i = 0; i < p.tokens.length; i++) {
        const t = p.tokens[i];
        if (i === key.tokenIndex) {
          highlightStart = contextSentence.length;
          contextSentence += t.bunsetsu.surface;
          highlightEnd = contextSentence.length;
        } else {
          contextSentence += t.bunsetsu.surface;
        }
      }

      // 获取多词典释义列表
      const dictDefs = await lookupFn(token.bunsetsu.head_word.base_form, token.bunsetsu.head_word.reading);

      exportEntries.push({
        surface: token.bunsetsu.surface,
        base_form: token.bunsetsu.head_word.base_form,
        reading: token.bunsetsu.head_word.reading,
        pos: token.bunsetsu.head_word.pos.major,
        grammar_tags: token.bunsetsu.grammar_tags.map((t) => t.name_ja),
        context_sentence: contextSentence,
        context_highlight: [highlightStart, highlightEnd],
        definitions: dictDefs,
        jlpt_levels: [...new Set(token.bunsetsu.grammar_tags.map((t) => t.jlpt_level).filter((level): level is number => level !== null))].sort((a, b) => a - b),
        user_note: notes.value[`${key.paragraphId}-${key.tokenIndex}`] ?? "",
        char_range: token.bunsetsu.char_range,
      });
    }

    try {
      // 调用 Rust 端导出接口，做 JSON 处理，返回完整 JSON 字符串
      const jsonStr = await invoke<string>("export_selected", {
        sourceText,
        selectedEntries: exportEntries,
      });
      return jsonStr;
    } catch (err) {
      console.error("Export Error:", err);
      throw err;
    }
  }

  return {
    selectedKeys,
    updateNote,
    toggleSelect,
    markAsKnown,
    markAsUnknown,
    exportSelected,
  };
}
