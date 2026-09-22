'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { presentation } = require('../MacApp/Resources/sphere/watch-link.js');

test('non-phone shells omit the watch card', () => {
  assert.equal(presentation({ supported: false }), null);
  assert.equal(presentation(null), null);
});

test('an unpaired phone says pairing is the missing step', () => {
  const view = presentation({ supported: true, paired: false, appInstalled: false, lastSent: '' });
  assert.equal(view.state, 'unpaired');
  assert.match(view.title, /No paired Apple Watch/);
  assert.match(view.detail, /Pair a watch/);
});

test('a paired phone distinguishes an absent watch app', () => {
  const view = presentation({ supported: true, paired: true, appInstalled: false, lastSent: '' });
  assert.equal(view.state, 'app-absent');
  assert.match(view.title, /not installed/);
  assert.match(view.detail, /iPhone Watch app/);
});

test('an installed watch distinguishes pending from sent address handoff', () => {
  const pending = presentation({ supported: true, paired: true, appInstalled: true, lastSent: '' });
  assert.equal(pending.state, 'pending');
  assert.match(pending.title, /not sent yet/);

  const sent = presentation({
    supported: true,
    paired: true,
    appInstalled: true,
    lastSent: 'door.example',
  });
  assert.equal(sent.state, 'sent');
  assert.match(sent.title, /queued for the watch/);
  assert.match(sent.detail, /door\.example/);
});
