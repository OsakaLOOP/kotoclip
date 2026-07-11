import { onMounted, onBeforeUnmount, Ref, watch } from "vue";

export function useScrollFocus(scrollContainerRef: Ref<HTMLElement | null>) {
  let rafId: number | null = null;

  function updateFocus() {
    const container = scrollContainerRef.value;
    if (!container) return;

    // 不再设置滚动时的段落透明度，将所有段落透明度重置为默认值
    const blocks = container.querySelectorAll(".paragraph-block");
    blocks.forEach((el) => {
      (el as HTMLElement).style.opacity = "";
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
    triggerUpdate,
  };
}
