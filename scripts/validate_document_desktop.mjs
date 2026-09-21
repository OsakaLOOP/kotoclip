// 验证真实桌面中的正文范围切换、查询目标和快速更换文本。
import { readFile, writeFile } from 'node:fs/promises';
import { connectDesktop } from './desktop_cdp.mjs';

const { send, evaluate, close } = await connectDesktop();
const source = JSON.parse(await readFile('data/validation/behavior/ginza.json', 'utf8'));
const news = source.segments.find(item => item.id === 'news').text;
const contraction = source.segments.find(item => item.id === 'contraction').text;

async function open(text) {
  await evaluate(`(() => { const input = document.querySelector('textarea'); input.value = ${JSON.stringify(text)}; input.dispatchEvent(new Event('input', {bubbles:true})); })()`);
  await evaluate(`document.querySelector('.primary-button').click()`);
}
async function waitFor(expression, message, timeout = 90000) {
  return evaluate(`(async () => { const deadline = Date.now() + ${timeout}; while (Date.now() < deadline) {
    const value = (${expression}); if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 100));
  } throw new Error(${JSON.stringify(message)}); })()`);
}

try {
  await open(news + '\n' + contraction);
  await waitFor(`document.querySelectorAll('.session-toolbar option').length === 2 && document.querySelector('.token-text')?.textContent.trim() === ${JSON.stringify(news)}`, '首段正文未显示');
  await waitFor(`document.querySelector('.session-toolbar')?.textContent.includes('结构 2/2')`, '两段结构分析未完成');
  const mixed = await evaluate(`document.querySelector('.result-toolbar').textContent`);
  if (!mixed.includes('cwj') || !mixed.includes('csj')) throw new Error('叙述与引语的独立词典未显示');
  await evaluate(`(() => { const select = document.querySelector('.session-toolbar select'); select.value = '1'; select.dispatchEvent(new Event('change', {bubbles:true})); })()`);
  await waitFor(`document.querySelector('.token-text')?.textContent.trim() === ${JSON.stringify(contraction)} && document.querySelectorAll('.word').length > 0`, '段落跳转或正文范围错误');
  await evaluate(`document.querySelector('.word').click()`);
  await waitFor(`document.querySelector('.inspector h2').textContent !== '词语详情' && !document.querySelector('.inspector .spin')`, '查词未完成');
  const selection = await evaluate(`document.querySelector('.inspector h2').textContent`);
  await open(news);
  await waitFor(`!document.querySelector('.primary-button').disabled && document.querySelector('.session-toolbar')`, '新文本打开超时');
  await open(contraction);
  await waitFor(`document.querySelector('.token-text')?.textContent.trim() === ${JSON.stringify(contraction)} && document.querySelector('.session-toolbar')?.textContent.includes('结构 1/1')`, '快速切换后出现旧结果');
  await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: false });
  const viewport = await evaluate(`({width: innerWidth, body: document.documentElement.scrollWidth})`);
  if (viewport.body > viewport.width) throw new Error('会话工具栏在窄窗口溢出');
  const report = { desktop: true, units: 2, mixed_registers: true, exact_visible_text: true, selection,
    range_query: true, rapid_replacement: true, viewport };
  const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await writeFile('experiments/session-desktop.png', Buffer.from(screenshot.data, 'base64'));
  await writeFile('data/validation/behavior/session-desktop.json', JSON.stringify(report, null, 2) + '\n', 'utf8');
  console.log(JSON.stringify(report));
} finally {
  await send('Emulation.clearDeviceMetricsOverride');
  close();
}
