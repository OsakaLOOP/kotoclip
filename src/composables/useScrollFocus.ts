import { onMounted, onBeforeUnmount, Ref, watch } from "vue";

export function useScrollFocus(scrollContainerRef: Ref<HTMLElement | null>) {
  let rafId: number | null = null;

  function updateFocus() {
    const container = scrollContainerRef.value;
    if (!container) return;

    // 如果处于墨水屏模式，直接跳过渐隐处理（在 CSS 中已有 eink-mode 覆盖，但这里也避免不必要的 DOM 操作）
    if (document.body.classList.contains("eink-mode")) {
      const blocks = container.querySelectorAll(".paragraph-block");
      blocks.forEach((el) => {
        (el as HTMLElement).style.opacity = "";
      });
      return;
    }

    const containerRect = container.getBoundingClientRect();
    const containerCenterY = containerRect.top + containerRect.height / 2;

    const blocks = container.querySelectorAll(".paragraph-block");
    if (blocks.length === 0) return;

    let minDistance = Infinity;
    let closestIndex = -1;

    // 1. 寻找最靠近视口中心的段落
    blocks.forEach((el) => {
      const htmlEl = el as HTMLElement;
      const rect = htmlEl.getBoundingClientRect();
      const elCenterY = rect.top + rect.height / 2;
      const distance = Math.abs(elCenterY - containerCenterY);

      if (distance < minDistance) {
        minDistance = distance;
        const dataIndex = htmlEl.getAttribute("data-index");
        closestIndex = dataIndex ? parseInt(dataIndex, 10) : -1;
      }
    });

    if (closestIndex === -1) return;

    // 2. 根据与 Focus 段落的实际段落索引差值，设置对应的透明度
    // 用户指定：Focus 为 1.0, +-1 为 0.7, 更远的为 0.5
    blocks.forEach((el) => {
      const htmlEl = el as HTMLElement;
      const dataIndex = htmlEl.getAttribute("data-index");
      if (!dataIndex) return;

      const idx = parseInt(dataIndex, 10);
      const diff = Math.abs(idx - closestIndex);

      let opacity = "0.5";
      if (diff === 0) {
        opacity = "1";
      } else if (diff === 1) {
        opacity = "0.7";
      }

      if (htmlEl.style.opacity !== opacity) {
        htmlEl.style.opacity = opacity;
      }
    });
  }

  function handleScroll() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
    }
    rafId = requestAnimationFrame(() => {
      updateFocus();
      rafId = null;
    });
  }

  // 提供手动触发的接口，以备在数据加载/重排后立即刷新
  function triggerUpdate() {
    requestAnimationFrame(updateFocus);
  }

  // 监听容器 DOM 元素的变化，动态绑定/解绑事件，适配 v-if/v-else 渲染
  watch(scrollContainerRef, (newEl, oldEl) => {
    if (oldEl) {
      oldEl.removeEventListener("scroll", handleScroll);
    }
    if (newEl) {
      newEl.addEventListener("scroll", handleScroll, { passive: true });
      triggerUpdate();
    }
  });

  onMounted(() => {
    // 监听窗口大小改变以重新计算
    window.addEventListener("resize", handleScroll, { passive: true });
  });

  onBeforeUnmount(() => {
    const container = scrollContainerRef.value;
    if (container) {
      container.removeEventListener("scroll", handleScroll);
    }
    window.removeEventListener("resize", handleScroll);
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
    }
  });

  return {
    updateFocus,
    triggerUpdate,
  };
}
