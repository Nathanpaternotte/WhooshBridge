// Runs only in the official top-level page. Never changes consent decisions.
(() => {
  if (window.top !== window || location.origin !== 'https://event.mywhoosh.com') return;
  // Observe only refusal status; the site's actions swallow HTTP exceptions.
  if (typeof XMLHttpRequest !== 'undefined') {
    const open = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url, ...args) {
      let activityRequest = false;
      try { const u = new URL(url, location.origin); activityRequest = u.origin === 'https://service14.mywhoosh.com' && u.pathname.startsWith('/v2/rider/profile/'); } catch (_) {}
      this.addEventListener('load', () => {
        if (activityRequest && [401, 403].includes(this.status)) window.__WHOOSH_AUTH_REFUSED__ = true;
      });
      return open.call(this, method, url, ...args);
    };
  }
  let reported = false;
  let offered = false;
  let cmpReported = false;
  let ticks = 0;
  const diagnostic = code => window.ipc.postMessage(JSON.stringify({type: 'diagnostic', code}));
  const cmpReady = () => window.UC_UI && typeof window.UC_UI.isInitialized === 'function' && window.UC_UI.isInitialized();

  function openPreferences() {
    if (!cmpReady() || typeof window.UC_UI.showSecondLayer !== 'function') return;
    try {
      Promise.resolve(window.UC_UI.showSecondLayer()).catch(() => diagnostic('consent_panel_error'));
      diagnostic('consent_panel_opened');
    } catch (_) { diagnostic('consent_panel_error'); }
  }

  setInterval(() => {
    ticks++;
    const root = document.getElementById('app');
    const store = root && root.__vue__ && root.__vue__.$store;
    if (!reported && store && store.getters['auth/loggedIn'] && localStorage.getItem('webToken')) {
      reported = true;
      window.ipc.postMessage(JSON.stringify({type: 'ready'}));
    }
    if (!reported && window.__WHOOSH_RESTORE__ && store && ticks >= 6 && !localStorage.getItem('webToken')) {
      reported = true;
      window.ipc.postMessage(JSON.stringify({type: 'auth_required'}));
    }
    if (reported || !document.body) return;

    let button = document.getElementById('whoosh-bridge-cookie-preferences');
    if (!button) {
      button = document.createElement('button');
      button.id = 'whoosh-bridge-cookie-preferences';
      button.type = 'button';
      button.style.cssText = 'position:fixed;bottom:16px;left:16px;z-index:2147483000;padding:12px 16px;border:1px solid #777;border-radius:8px;background:#fff;color:#111;font:14px system-ui;cursor:pointer;max-width:90vw';
      button.addEventListener('click', openPreferences);
      document.body.appendChild(button);
    }
    const available = cmpReady();
    button.disabled = !available;
    button.textContent = window.__WHOOSH_ENGLISH__ ? (available ? 'MyWhoosh cookie settings' : ticks < 40 ? 'Loading cookie settings…' : 'Cookie settings unavailable on MyWhoosh') : available ? 'Réglages des cookies MyWhoosh' : ticks < 40 ? 'Chargement des réglages des cookies…' : 'Réglages des cookies indisponibles sur MyWhoosh';
    if (available && !cmpReported) { cmpReported = true; diagnostic('cmp_ready'); }
    if (!available && ticks === 40) diagnostic('cmp_not_ready');
    // The site's public configuration disables both the initial banner and its
    // privacy button. Offer the official preferences once when CAPTCHA is blocked.
    // Reopening the panel neither accepts services nor loads CAPTCHA without consent.
    if (available && !offered && document.getElementById('uc-recaptcha-mock')) {
      offered = true;
      openPreferences();
    }
  }, 500);
})();
