<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import ReaderView from "./components/ReaderView.vue";
import { floatDebug } from "./explanation/floatDebug";

const isInsider = import.meta.env.VITE_BUILD_CHANNEL === "insider";
const FloatDebugOverlay = import.meta.env.DEV && floatDebug.enabled
  ? defineAsyncComponent(() => import("./components/dev/FloatDebugOverlay.vue"))
  : null;
</script>

<template>
  <aside v-if="isInsider" class="insider-notice" role="note">
    <strong>内部预览版本</strong>，不代表最终成品。词典来源：三省堂《Super大辞林 3.1》；NLP 库来源：Vibrato 0.5.2 fork / IPADIC。
    不得商业利用或二次分发；如因此造成侵权，作者不负责任。
  </aside>
  <div class="reader-shell">
    <ReaderView />
  </div>
  <FloatDebugOverlay v-if="FloatDebugOverlay" />
</template>

<style>
/* 导入全局样式系统 */
@import "./styles/main.css";
@import "./styles/capsule.css";
@import "./styles/eink.css";

.insider-notice {
  flex: 0 0 auto;
  padding: 6px 14px;
  background: #fff3cd;
  color: #5f4500;
  border-bottom: 1px solid #e5c766;
  font-size: 11px;
  line-height: 1.5;
  text-align: center;
}

.reader-shell {
  flex: 1;
  min-height: 0;
}

.reader-shell .reader-container {
  height: 100%;
}
</style>
