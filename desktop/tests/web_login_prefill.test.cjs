const {test} = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const source = fs.readFileSync(path.join(__dirname, '../src/web_login_prefill.js'), 'utf8');
function setup(origin = 'https://event.mywhoosh.com', pathname = '/auth/login/') {
  let tick, stopped = false, available = false;
  class Input {
    constructor() { this.events = []; this.disabled = false; }
    set value(v) { this.content = v; }
    dispatchEvent(e) { this.events.push(e.type); }
    closest() { return {querySelector: () => email}; }
  }
  const email = new Input(), password = new Input();
  const window = {}; window.top = window;
  const prefill = vm.runInNewContext(`(${source})`, {
    window, location: {origin, pathname}, HTMLInputElement: Input,
    document: {querySelector: () => available ? password : null},
    Event: class {constructor(type) {this.type = type;}},
    setInterval: fn => { tick = fn; return 1; }, clearInterval: () => {stopped = true;},
  });
  prefill('name+é@example.com', 'quotes"\\ and <script>');
  return {email, password, tick: () => {if (!stopped) tick?.();}, show: () => {available = true;}, stopped: () => stopped, scheduled: () => !!tick};
}
test('waits for the login form and fills Vue-compatible inputs exactly once', () => {
  const app = setup(); app.tick(); assert.equal(app.email.content, undefined);
  app.show(); app.tick();
  assert.equal(app.email.content, 'name+é@example.com');
  assert.equal(app.password.content, 'quotes"\\ and <script>');
  assert.deepEqual(app.email.events, ['input', 'change']);
  assert.equal(app.stopped(), true);
  app.password.value = 'user correction'; app.tick();
  assert.equal(app.password.content, 'user correction');
});
test('never fills another origin or registration route', () => {
  const other = setup('https://example.com'); assert.equal(other.scheduled(), false);
  const signup = setup('https://event.mywhoosh.com', '/auth/register/');
  signup.show(); signup.tick(); assert.equal(signup.email.content, undefined);
});
test('does not interact with disabled fields and eventually drops the prefill', () => {
  const app = setup(); app.show(); app.email.disabled = true;
  for (let i = 0; i < 121; i++) app.tick();
  assert.equal(app.email.content, undefined); assert.equal(app.stopped(), true);
});
