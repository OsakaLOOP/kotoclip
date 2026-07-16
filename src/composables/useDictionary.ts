import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { DictionaryLookup, DictionarySettings } from "../types";

export function useDictionary() {
  const dictionaryResults = ref<DictionaryLookup | null>(null);
  const isSearching = ref(false);
  const dictionarySettings = ref<DictionarySettings>({
    available_dictionaries: [],
    default_dictionary: null,
    dictionary_order: [],
  });

  function priorityList() {
    return dictionarySettings.value.dictionary_order.length
      ? dictionarySettings.value.dictionary_order
      : dictionarySettings.value.available_dictionaries;
  }

  async function loadDictionarySettings() {
    dictionarySettings.value = await invoke<DictionarySettings>("get_dictionary_settings");
    return dictionarySettings.value;
  }

  async function setDictionaryOrder(order: string[]) {
    dictionarySettings.value = await invoke<DictionarySettings>("set_dictionary_order", { order });
    return dictionarySettings.value;
  }

  /**
   * 根据原形与读音检索词典释义
   * @param word 辞书形原形
   * 默认词典排在检索结果首位，其余已加载词典保持稳定顺序。
   */
  async function lookupWord(word: string, reading?: string) {
    if (!word) {
      dictionaryResults.value = null;
      return null;
    }

    isSearching.value = true;
    try {
      const results = await invoke<DictionaryLookup>("lookup_word", {
        word,
        reading,
        priorityList: priorityList(),
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
    dictionarySettings,
    loadDictionarySettings,
    setDictionaryOrder,
    lookupWord,
    chooseDictionaryTarget,
  };
}
