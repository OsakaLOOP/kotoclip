import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { AnnotatedToken } from "../types";

export interface Paragraph {
  id: number;
  tokens: AnnotatedToken[];
}

export function useTokenization() {
  const paragraphs = ref<Paragraph[]>([]);
  const isAnalyzing = ref(false);
  const errorMsg = ref<string | null>(null);

  /**
   * 分析整页文本
   */
  async function analyzeText(text: string) {
    if (!text.trim()) {
      paragraphs.value = [];
      return;
    }

    isAnalyzing.value = true;
    errorMsg.value = null;

    try {
      // 调用 Tauri 命令进行分词与用户画像评分标注
      const allTokens = await invoke<AnnotatedToken[]>("analyze_text", { text });
      
      // 根据换行符切分为段落结构，方便虚拟列表高效渲染
      const tempParagraphs: Paragraph[] = [];
      let currentTokens: AnnotatedToken[] = [];
      let paragraphId = 0;

      for (const token of allTokens) {
        // 判断是否为换行符
        if (token.bunsetsu.surface === "\n" || token.bunsetsu.surface === "\r\n") {
          tempParagraphs.push({
            id: paragraphId++,
            tokens: currentTokens,
          });
          currentTokens = [];
        } else {
          currentTokens.push(token);
        }
      }

      // 将剩余的最后一组 tokens 压入
      if (currentTokens.length > 0 || tempParagraphs.length === 0) {
        tempParagraphs.push({
          id: paragraphId++,
          tokens: currentTokens,
        });
      }

      paragraphs.value = tempParagraphs;
    } catch (err: any) {
      errorMsg.value = err.toString() || "分词分析失败";
      console.error("Tokenization Error:", err);
    } finally {
      isAnalyzing.value = false;
    }
  }

  /**
   * 合并两个或多个文节 (添加自定义合并规则)
   */
  async function mergeTokens(surfaces: string[]) {
    try {
      await invoke("add_merge_rule", { parts: surfaces });
    } catch (err: any) {
      console.error("Merge Rule Register Error:", err);
      throw err;
    }
  }

  return {
    paragraphs,
    isAnalyzing,
    errorMsg,
    analyzeText,
    mergeTokens,
  };
}
