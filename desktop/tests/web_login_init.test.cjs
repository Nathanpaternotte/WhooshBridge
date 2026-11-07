const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../src/web_login_init.js'), 'utf8');

function setup({cmp = true, captcha = true, origin = 'https://event.mywhoosh.com', restore = false, token = 'PRIVATE_SESSION_TOKEN'} = {}) {
  let tick, opens = 0;
  const messages = [], elements = new Map();
  const store = {getters: {'auth/loggedIn': false}};
  elements.set('app', {__vue__: {$store: store}});
  if (captcha) elements.set('uc-recaptcha-mock', {});
  const UC_UI = cmp ? {isInitialized: () => true, showSecondLayer: () => { opens++; }} : undefined;
  const window = {__WHOOSH_RESTORE__: restore, UC_UI, ipc: {postMessage: raw => messages.push(JSON.parse(raw))}};
  window.top = window;
  const document = {
    body: {appendChild: el => elements.set(el.id, el)},
    getElementById: id => elements.get(id),
    createElement: () => ({style: {}, addEventListener: function(event, fn) {this[event] = fn;}}),
  };
  vm.runInNewContext(script, {window, document, location: {origin}, localStorage: {getItem: () => token}, setInterval: fn => {tick = fn;}});
  return {tick: () => tick?.(), hasTimer: () => !!tick, messages, elements, store, opens: () => opens};
}

test('offers official preferences once; does not set consent or leak tokens', () => {
  const app = setup();
  app.tick(); app.tick(); app.tick();
  assert.equal(app.opens(), 1);
  const button = app.elements.get('whoosh-bridge-cookie-preferences');
  assert.equal(button.textContent, 'Réglages des cookies MyWhoosh');
  button.click();
  assert.equal(app.opens(), 2);
  app.store.getters['auth/loggedIn'] = true;
  app.tick(); app.tick();
  assert.equal(app.messages.filter(m => m.type === 'ready').length, 1);
  assert.ok(!JSON.stringify(app.messages).includes('PRIVATE_SESSION_TOKEN'));
});

test('missing CMP is diagnosed and never replaced or bypassed', () => {
  const app = setup({cmp: false});
  for (let i = 0; i < 41; i++) app.tick();
  assert.equal(app.opens(), 0);
  assert.equal(app.elements.get('whoosh-bridge-cookie-preferences').disabled, true);
  assert.equal(app.messages.filter(m => m.code === 'cmp_not_ready').length, 1);
});

test('does not show preferences automatically when CAPTCHA is not blocked', () => {
  const app = setup({captcha: false});
  app.tick();
  assert.equal(app.opens(), 0);
});

test('does not run on third-party pages', () => {
  const app = setup({origin: 'https://www.google.com'});
  assert.equal(app.hasTimer(), false);
});


test('restores saved session silently; asks login only when storage is absent', () => {
  const saved = setup({restore: true});
  saved.store.getters['auth/loggedIn'] = true;
  for (let i = 0; i < 8; i++) saved.tick();
  assert.equal(saved.messages.filter(m => m.type === 'ready').length, 1);
  assert.equal(saved.messages.filter(m => m.type === 'auth_required').length, 0);
  const empty = setup({restore: true, token: null});
  for (let i = 0; i < 8; i++) empty.tick();
  assert.equal(empty.messages.filter(m => m.type === 'auth_required').length, 1);
  const pending = setup({restore: true});
  for (let i = 0; i < 100; i++) pending.tick();
  assert.equal(pending.messages.filter(m => m.type === 'auth_required').length, 0);
});
