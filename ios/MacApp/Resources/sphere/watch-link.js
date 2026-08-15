'use strict';

// Pure presentation seam for T-171. The bridge supplies phone-local WatchConnectivity facts;
// this function decides only how to say them. CommonJS export keeps every state fixture-testable
// without loading WebKit, while the browser global is consumed by the shared sphere console.
(function install(root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  root.FamiliarWatchLink = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function makeWatchLink() {
  function presentation(watch) {
    if (!watch || watch.supported !== true) return null;

    if (!watch.paired) {
      return {
        state: 'unpaired',
        title: 'No paired Apple Watch detected',
        detail: 'Pair a watch with this iPhone, then tap re-link here.',
        tone: '#ff8fa3',
        border: 'rgba(255,143,163,0.34)',
      };
    }
    if (!watch.appInstalled) {
      return {
        state: 'app-absent',
        title: 'Familiar is not installed on the watch',
        detail: 'Open the iPhone Watch app → My Watch → Available Apps, install Familiar, then tap re-link.',
        tone: '#ffcf9e',
        border: 'rgba(255,178,102,0.38)',
      };
    }

    const lastSent = String(watch.lastSent || '').trim();
    if (!lastSent) {
      return {
        state: 'pending',
        title: 'Watch app installed; address not sent yet',
        detail: 'Tap re-link, then open Familiar on the watch.',
        tone: '#9dc0ff',
        border: 'rgba(157,192,255,0.34)',
      };
    }

    return {
      state: 'sent',
      title: 'Familiar address queued for the watch',
      detail: `Last sent from this iPhone: ${lastSent}. Open Familiar on the watch to finish joining.`,
      tone: '#7ff0c0',
      border: 'rgba(127,240,192,0.32)',
    };
  }

  return Object.freeze({ presentation });
});
