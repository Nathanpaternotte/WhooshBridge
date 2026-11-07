// Credentials arrive through the private process pipe, never argv or a URL.
(email, password) => {
  if (window.top !== window || location.origin !== 'https://event.mywhoosh.com') return;
  let attempts = 0;
  const timer = setInterval(() => {
    if (++attempts > 120) { clearInterval(timer); email = password = ''; return; }
    if (!/^\/auth\/login\/?$/.test(location.pathname)) return;
    const secret = document.querySelector('input[type="password"]');
    const form = secret && secret.closest('form');
    const address = form && form.querySelector('input[type="email"], input[name="email"], input#email');
    if (!secret || !address || secret.disabled || address.disabled) return;
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    for (const [field, value] of [[address, email], [secret, password]]) {
      setter.call(field, value);
      field.dispatchEvent(new Event('input', {bubbles: true}));
      field.dispatchEvent(new Event('change', {bubbles: true}));
    }
    clearInterval(timer);
    email = password = '';
    // Never submit, change consent, or resolve CAPTCHA on the user's behalf.
  }, 500);
}
