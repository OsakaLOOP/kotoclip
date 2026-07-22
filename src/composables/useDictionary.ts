import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { DictionaryLookup, DictionaryLookupRequest, DictionarySettings } from "../types";

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

  /** 按根查询与活动表记加载稳定的“表记 × 词典”矩阵。 */
  async function lookupWord(request: DictionaryLookupRequest) {
    if (!request.word) {
      dictionaryResults.value = null;
      return null;
    }

    isSearching.value = true;
    try {
      const results = await invoke<DictionaryLookup>("lookup_word", {
        word: request.word,
        observedForm: request.observedForm,
        reading: request.reading,
        pos: request.pos,
        selectedForm: request.selectedForm,
        priorityList: priorityList(),
        background: request.background ?? false,
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

  return {
    dictionaryResults,
    isSearching,
    dictionarySettings,
    loadDictionarySettings,
    setDictionaryOrder,
    lookupWord,
  };
}
