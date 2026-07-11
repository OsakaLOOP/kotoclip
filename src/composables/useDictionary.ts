import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { DictionaryLookup } from "../types";

export function useDictionary() {
  const dictionaryResults = ref<DictionaryLookup | null>(null);
  const isSearching = ref(false);
  
  // 默认词典优先级顺序列表
  const defaultPriority = ["大辞林", "新明解国語辞典", "三省堂国語辞典", "広辞苑"];

  /**
   * 根据原形与读音检索词典释义
   * @param word 辞书形原形
   * @param priorityList 用户偏好的词典优先级列表
   */
  async function lookupWord(word: string, reading?: string, priorityList: string[] = defaultPriority) {
    if (!word) {
      dictionaryResults.value = null;
      return null;
    }

    isSearching.value = true;
    try {
      const results = await invoke<DictionaryLookup>("lookup_word", {
        word,
        reading,
        priorityList,
      });
      dictionaryResults.value = results;
      return results;
    } catch (err) {
      console.error("Dictionary Lookup Error:", err);
      dictionaryResults.value = null;
      return null;
    } finally {
      isSearching.value = false;
    }
  }

  async function chooseDictionaryTarget(query: string, reading: string | null, target: string) {
    await invoke("choose_dictionary_target", { query, reading, target });
    return lookupWord(query, reading ?? undefined);
  }

  return {
    dictionaryResults,
    isSearching,
    lookupWord,
    chooseDictionaryTarget,
  };
}
