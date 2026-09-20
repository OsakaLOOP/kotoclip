// 通过本机 WebView2 调试端口验证真实 Tauri 窗口。
import { writeFile } from 'node:fs/promises';

const pages = await fetch('http://127.0.0.1:9222/json/list').then(r => r.json());
const page = pages.find(p => p.title === 'Kotoclip');
if (!page) throw new Error('未找到 Kotoclip 桌面窗口');
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let sequence = 0;
const pending = new Map();
socket.onmessage = ({ data }) => {
  const value = JSON.parse(data);
  const request = pending.get(value.id);
  if (!request) return;
  pending.delete(value.id);
  if (value.error) request.reject(new Error(JSON.stringify(value.error)));
  else request.resolve(value.result);
};
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++sequence; pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const response = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
  return response.result.value;
}
try {
  const desktop = await evaluate(`Boolean(window.__TAURI_INTERNALS__)`);
  if (!desktop) throw new Error('验收页面未连接 Tauri IPC');
  await evaluate(`(async () => {
    const deadline = Date.now() + 90000;
    while (!document.querySelector('textarea')) {
      if (Date.now() >= deadline) throw new Error('桌面页面加载超时');
      await new Promise(resolve => setTimeout(resolve, 250));
    }
  })()`);
  await evaluate(`(() => {
    const input = document.querySelector('textarea');
    input.value = 'ラティメリアはきょろきょろと部屋を見渡すが、鏡の裏から覗かれてるとは気づかない。';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);
  await evaluate(`document.querySelector('.primary-button').click()`);
  const state = await evaluate(`(async () => {
    const deadline = Date.now() + 90000;
    while (Date.now() < deadline) {
      const panel = document.querySelector('.provider-panel');
      if (panel.textContent.includes('ginza：完成') && panel.textContent.includes('kwja：完成'))
        return { text: panel.textContent, words: document.querySelectorAll('.word').length };
      if (panel.textContent.includes('失败')) throw new Error(panel.textContent);
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    throw new Error('桌面结构分析超时');
  })()`);
  if (state.words < 1) throw new Error('正文尚未展示');
  await evaluate(`document.querySelector('.provider-heading button').click()`);
  await evaluate(`document.querySelector('.provider-heading button').click()`);
  const cancellation = await evaluate(`(async () => {
    const deadline = Date.now() + 10000;
    while (Date.now() < deadline) {
      if (document.querySelector('.provider-panel').textContent.includes('已取消')) return true;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error('桌面取消操作未生效');
  })()`);
  await evaluate(`document.querySelector('.provider-heading button').click()`);
  await evaluate(`(async () => {
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      const text = document.querySelector('.provider-panel').textContent;
      if (text.includes('ginza：完成') && text.includes('kwja：完成')) return;
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    throw new Error('桌面取消后重试超时');
  })()`);
  await evaluate(`(() => { const select = document.querySelector('.provider-filters select'); select.value = 'kwja'; select.dispatchEvent(new Event('change', {bubbles:true})); })()`);
  const kwja = await evaluate(`document.querySelector('.source-nodes').textContent`);
  if (!kwja.includes('ラティメリア')) throw new Error('KWJA 结构未显示');
  await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: false });
  const overflow = await evaluate(`({width: innerWidth, body: document.documentElement.scrollWidth})`);
  const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await writeFile('experiments/provider-desktop.png', Buffer.from(screenshot.data, 'base64'));
  const report = { desktop, words: state.words, ginza: true, kwja: true, cancellation_recovery: cancellation, kwja_structure: kwja, viewport: overflow };
  await writeFile('data/validation/behavior/desktop.json', JSON.stringify(report, null, 2) + '\n', 'utf8');
  console.log(JSON.stringify(report));
} finally {
  await send('Emulation.clearDeviceMetricsOverride');
  socket.close();
}
