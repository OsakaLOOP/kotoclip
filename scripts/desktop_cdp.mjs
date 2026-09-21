// 连接本机真实 Tauri 窗口，供桌面场景共用。
export async function connectDesktop() {
  let page;
  const deadline = Date.now() + 90000;
  while (!page && Date.now() < deadline) {
    const pages = await fetch('http://127.0.0.1:9222/json/list').then(response => response.json()).catch(() => []);
    page = pages.find(page => page.title === 'Kotoclip');
    if (!page) await new Promise(resolve => setTimeout(resolve, 250));
  }
  if (!page) throw new Error('未找到 Kotoclip 桌面窗口');
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  socket.onmessage = ({ data }) => {
    const value = JSON.parse(data), request = pending.get(value.id);
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
  return { send, evaluate, close: () => socket.close() };
}
