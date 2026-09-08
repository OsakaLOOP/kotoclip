import assert from 'node:assert/strict';
import test from 'node:test';
import { compileReaderDocument } from './document.ts';

test('完整词注音和重复符号清洗后保持后续章节坐标', () => {
  const doc = compileReaderDocument('可愛らしい《かわいらしい》神々《かみがみ》かな《かな》\n\n## 次章');
  const prepared = '可愛らしい神々かな《かな》';
  assert.equal(doc.blocks[0].charRange[1], Array.from(prepared).length);
  assert.equal(doc.chapters[0].charOffset, Array.from(prepared).length + 2);
});
