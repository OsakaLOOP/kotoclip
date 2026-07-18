<script setup lang="ts">
import { defineAsyncComponent, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import ReaderView from "./components/ReaderView.vue";
import OnboardingExperience from "./components/onboarding/OnboardingExperience.vue";
import { floatDebug } from "./explanation/floatDebug";

const isInsider = import.meta.env.VITE_BUILD_CHANNEL === "insider";
const onboardingStorageKey = "kotoclip:onboarding:v1";
const onboardingPreview = new URLSearchParams(window.location.search).get("onboarding") === "1";

function shouldShowOnboarding(): boolean {
  if (onboardingPreview) return true;
  try {
    return window.localStorage.getItem(onboardingStorageKey) !== "completed";
  } catch {
    return true;
  }
}

const showOnboarding = ref(shouldShowOnboarding());
const FloatDebugOverlay = import.meta.env.DEV && floatDebug.enabled
  ? defineAsyncComponent(() => import("./components/dev/FloatDebugOverlay.vue"))
  : null;

function completeOnboarding() {
  if (!onboardingPreview) {
    try {
      window.localStorage.setItem(onboardingStorageKey, "completed");
    } catch {
      // 本地存储不可用时仍允许用户进入阅读器。
    }
  }
  showOnboarding.value = false;
}

onMounted(() => {
  const bootTime = (window as any).__boot_start_time || Date.now();
  const mainTime = (window as any).__main_loaded_time || Date.now();
  const appMountedTime = Date.now();

  console.log(
    "[时间戳] App.vue 根视图完全挂载: %d (延迟: %dms)",
    appMountedTime,
    appMountedTime - bootTime
  );

  invoke("log_ui_timestamps", {
    bootTime,
    mainLoaded: mainTime,
    appMounted: appMountedTime,
  }).catch((err) => {
    console.error("无法发送时间戳到后端:", err);
  });
});
</script>

<template>
  <aside v-if="isInsider" class="insider-notice" role="note">
    <strong>内部预览版本</strong>，不代表最终成品。词典来源：三省堂《Super大辞林 3.1》、小学馆《日中辞典》第 3 版、CROWN《日中辞典》；NLP 库来源：Vibrato 0.5.2 fork / IPADIC。
    不得商业利用或二次分发；如因此造成侵权，作者不负责任。
  </aside>
  <div class="reader-shell">
    <ReaderView v-show="!showOnboarding" />
    <OnboardingExperience
      v-if="showOnboarding"
      @complete="completeOnboarding"
    />
  </div>
  <FloatDebugOverlay v-if="FloatDebugOverlay && !showOnboarding" />
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
  position: relative;
  overflow: hidden;
}

.reader-shell .reader-container {
  height: 100%;
}
</style>
