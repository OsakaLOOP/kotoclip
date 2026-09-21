import assert from 'node:assert/strict';
import test from 'node:test';
import { openSession, mergeSession } from './session.ts';

const plan = { id: 'book', text_version: 'text-v1', units: [] };
const initial = { schema: 'kotoclip.document-update.v1', session_id: 's1', text_version: 'text-v1', generation: 1,
  base_revision: null, revision: 0, snapshot: true, paused: false, progress: { total: 1 }, changes: [{ unit_id: 'u1', stage: 'pending' }] };

test('按基准版本合并结构追加并拒绝跨会话迟到结果', () => {
  const state = openSession(plan, initial);
  const update = { ...initial, base_revision: 0, revision: 1, snapshot: false, changes: [{ unit_id: 'u1', stage: 'complete' }] };
  assert.equal(mergeSession(state, update).units.u1.stage, 'complete');
  assert.equal(mergeSession(state, { ...update, session_id: 's2' }), state);
  assert.equal(mergeSession(state, { ...update, text_version: 'text-v2' }), state);
  assert.equal(mergeSession(state, { ...update, base_revision: 5 }), null);
});

test('取消后的新代次快照替换状态且排除旧任务', () => {
  const state = openSession(plan, initial);
  const cancelled = mergeSession(state, { ...initial, generation: 2, revision: 2, paused: true });
  assert.equal(cancelled.paused, true);
  assert.equal(mergeSession(cancelled, { ...initial, revision: 3 }), cancelled);
  assert.equal(mergeSession(cancelled, { ...initial, generation: 3, base_revision: 2, revision: 3, snapshot: false }), null);
});
