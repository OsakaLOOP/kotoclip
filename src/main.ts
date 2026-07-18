import { createApp } from "vue";
import App from "./App.vue";

(window as any).__main_loaded_time = Date.now();
console.log(
  "[时间戳] main.ts 入口脚本加载执行: %d (延迟: %dms)",
  (window as any).__main_loaded_time,
  (window as any).__main_loaded_time - (window as any).__boot_start_time
);

createApp(App).mount("#app");
