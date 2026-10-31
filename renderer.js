const header = document.querySelector('header');
const address = document.querySelector('#address');
const error = document.querySelector('#error');
let composing = false;
address.addEventListener('compositionstart', () => { composing = true; });
address.addEventListener('compositionend', () => { composing = false; });
document.querySelector('form').onsubmit = event => {
  event.preventDefault();
  if (!composing) window.browser.command('navigate', address.value);
};
window.browser.onPrompt(({open, url, error: message}) => {
  header.hidden = !open;
  error.textContent = message;
  error.hidden = !message;
  if (open) {
    if (!message) address.value = url;
    address.focus();
    if (!message) address.select();
  } else {
    address.value = url;
    address.blur();
  }
});
window.browser.onTheme(theme => {
  for (const [key, value] of Object.entries(theme)) {
    if (key !== 'mode') document.documentElement.style.setProperty(`--${key}`, value);
  }
});
