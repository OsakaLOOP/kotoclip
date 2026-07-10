import { ref, Ref } from "vue";
import { Paragraph } from "./useTokenization";

export function useDragMerge(
  paragraphs: Ref<Paragraph[]>,
  onMergeComplete: (surfaces: string[], paragraphId: number) => Promise<void>
) {
  const isDragging = ref(false);
  const startKey = ref<{ paragraphId: number; tokenIndex: number } | null>(null);
  const currentKey = ref<{ paragraphId: number; tokenIndex: number } | null>(null);

  /**
   * 判断一个 token 是否处于当前拖拽选中的范围中
   */
  function isTokenDragSelected(paragraphId: number, tokenIndex: number): boolean {
    if (!isDragging.value || !startKey.value || !currentKey.value) return false;
    if (startKey.value.paragraphId !== paragraphId || currentKey.value.paragraphId !== paragraphId) {
      return false;
    }

    const minIdx = Math.min(startKey.value.tokenIndex, currentKey.value.tokenIndex);
    const maxIdx = Math.max(startKey.value.tokenIndex, currentKey.value.tokenIndex);
    return tokenIndex >= minIdx && tokenIndex <= maxIdx;
  }

  /**
   * 处理段落的 mousedown 事件 (启动拖拽)
   */
  function handleMouseDown(e: MouseEvent, paragraphId: number) {
    const target = e.target as HTMLElement;
    const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
    if (!capsuleEl) return;

    // 排除已知词/标点
    if (capsuleEl.classList.contains("is-known") || capsuleEl.classList.contains("punctuation")) {
      return;
    }

    const tokenIndex = parseInt(capsuleEl.getAttribute("data-token-index") || "", 10);
    if (isNaN(tokenIndex)) return;

    isDragging.value = true;
    startKey.value = { paragraphId, tokenIndex };
    currentKey.value = { paragraphId, tokenIndex };

    // 阻止选择文字的默认行为以防干扰拖拽
    e.preventDefault();
  }

  /**
   * 处理段落的 mousemove 事件 (更新拖拽区间)
   */
  function handleMouseMove(e: MouseEvent, paragraphId: number) {
    if (!isDragging.value || !startKey.value) return;
    if (startKey.value.paragraphId !== paragraphId) return; // 不允许跨段落拖拽合并

    const target = e.target as HTMLElement;
    const capsuleEl = target.closest("[data-token-index]") as HTMLElement;
    if (!capsuleEl) return;

    // 排除已知词/标点
    if (capsuleEl.classList.contains("is-known") || capsuleEl.classList.contains("punctuation")) {
      return;
    }

    const tokenIndex = parseInt(capsuleEl.getAttribute("data-token-index") || "", 10);
    if (isNaN(tokenIndex)) return;

    currentKey.value = { paragraphId, tokenIndex };
  }

  /**
   * 处理全局 mouseup 事件 (结算拖拽合并)
   */
  async function handleMouseUp() {
    if (!isDragging.value) return;

    const start = startKey.value;
    const current = currentKey.value;

    isDragging.value = false;
    startKey.value = null;
    currentKey.value = null;

    if (!start || !current || start.paragraphId !== current.paragraphId) return;

    const paragraphId = start.paragraphId;
    const minIdx = Math.min(start.tokenIndex, current.tokenIndex);
    const maxIdx = Math.max(start.tokenIndex, current.tokenIndex);

    if (maxIdx - minIdx >= 1) {
      // 找到了跨越至少 2 个 token 的区间
      const p = paragraphs.value.find((para) => para.id === paragraphId);
      if (!p) return;

      // 提取这些 token 里的所有形态素 surface，保持先后物理顺序
      const surfacesToMerge: string[] = [];
      for (let idx = minIdx; idx <= maxIdx; idx++) {
        const token = p.tokens[idx];
        if (token) {
          for (const m of token.bunsetsu.morphemes) {
            surfacesToMerge.push(m.surface);
          }
        }
      }

      if (surfacesToMerge.length > 1) {
        // 触发合并回调 (调用 Rust 并重析)
        await onMergeComplete(surfacesToMerge, paragraphId);
      }
    }
  }

  return {
    isDragging,
    isTokenDragSelected,
    handleMouseDown,
    handleMouseMove,
    handleMouseUp,
  };
}
