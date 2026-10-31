const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {watchTheme} = require('../theme-watcher.cjs');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(check) {
  for (let i = 0; i < 100; i++) { if (check()) return; await delay(20); }
  assert.fail('theme update not delivered');
}
test('delivers creation, atomic edits, replacement, symlink and fallback changes; stays idle and closes', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'omawser-watch-'));
  const primary = path.join(root, 'state/current/theme/colors.toml');
  const fallback = path.join(root, 'config/current/theme/colors.toml');
  let calls = 0;
  let source;
  const watcher = watchTheme([primary, fallback], () => {
    calls++;
    source = undefined;
    for (const file of [primary, fallback]) { try { source = fs.readFileSync(file, 'utf8'); break; } catch {} }
  }, {debounceMs:20});
  const write = (file, text) => { fs.mkdirSync(path.dirname(file), {recursive:true}); fs.writeFileSync(file, text); };
  try {
    write(fallback, 'fallback'); await until(() => source === 'fallback');
    write(primary, 'first'); await until(() => source === 'first');
    write(primary + '.new', 'atomic'); fs.renameSync(primary + '.new', primary);
    await until(() => source === 'atomic');
    fs.renameSync(path.dirname(primary), path.dirname(primary) + '-old');
    write(primary, 'directory'); await until(() => source === 'directory');
    fs.rmSync(path.join(root, 'state/current'), {recursive:true});
    const targetA = path.join(root, 'targets/a');
    const targetB = path.join(root, 'targets/b');
    write(path.join(targetA, 'theme/colors.toml'), 'target-a');
    write(path.join(targetB, 'theme/colors.toml'), 'target-b');
    fs.symlinkSync(targetA, path.join(root, 'state/current'));
    await until(() => source === 'target-a');
    fs.symlinkSync(targetB, path.join(root, 'state/next'));
    fs.renameSync(path.join(root, 'state/next'), path.join(root, 'state/current'));
    await until(() => source === 'target-b');
    write(path.join(targetB, 'theme/colors.toml'), 'target-edited');
    await until(() => source === 'target-edited');
    fs.renameSync(targetB, targetB + '-old');
    write(path.join(targetB, 'theme/colors.toml'), 'target-recreated');
    await until(() => source === 'target-recreated');
    fs.unlinkSync(path.join(root, 'state/current')); await until(() => source === 'fallback');
    await delay(100); const idleCalls = calls;
    fs.writeFileSync(path.join(root, 'unrelated'), 'ignore');
    await delay(1200); assert.equal(calls, idleCalls, 'no callbacks while idle or on unrelated names');
    for (let i = 0; i < 20; i++) write(fallback, `burst-${i}`);
    await until(() => source === 'burst-19');
    await delay(100); assert.equal(calls, idleCalls + 1, 'burst has one debounced delivery');
    const closedCalls = calls;
    write(fallback, 'pending'); watcher.close(); watcher.close();
    await delay(100); assert.equal(calls, closedCalls, 'close cancels pending delivery');
  } finally { watcher.close(); fs.rmSync(root, {recursive:true, force:true}); }
});
