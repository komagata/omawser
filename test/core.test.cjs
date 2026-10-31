const {test} = require('node:test');
const assert = require('node:assert/strict');
const core = require('../core.cjs');
test('addresses accept web URLs and reject executable or local schemes', () => {
  assert.equal(core.address('x.com/home'), 'https://x.com/home');
  assert.equal(core.address(' http://localhost:8000 '), 'http://localhost:8000/');
  for (const url of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,hi', '']) assert.equal(core.address(url), null);
});
test('theme maps semantic colors and ignores invalid CSS values', () => {
  const t = core.theme('background = "#000000"\nforeground = "#e0ffd7"\naccent = "#91ff70"\nmuted = "bad; color:red"\nmode = "light"');
  assert.equal(t.background, '#000000');
  assert.equal(t.foreground, '#e0ffd7');
  assert.equal(t.accent, '#91ff70');
  assert.equal(t.mode, 'light');
  assert.match(t.muted, /^#[a-f0-9]{6}$/i);
});
