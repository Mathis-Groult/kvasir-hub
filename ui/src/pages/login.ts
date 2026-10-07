import { h, setChildren } from '../core/dom.ts';
import { command, failureFrom } from '../core/transport.ts';
import { formatAge } from '../core/format.ts';
import { brandMark } from '../ui/brand.ts';

/** Shows the sign-in form; resolves when the core accepts the credentials. */
export function showLogin(root: HTMLElement): Promise<void> {
  return new Promise((resolve) => {
    const error = h('p', { class: 'form-error', attrs: { role: 'alert' } });
    const user = h('input', { class: 'input', attrs: { id: 'login-user', name: 'username', type: 'text', autocomplete: 'username', autocapitalize: 'none', spellcheck: 'false', required: '' } });
    const pass = h('input', { class: 'input', attrs: { id: 'login-pass', name: 'password', type: 'password', autocomplete: 'current-password', required: '' } });
    const submit = h('button', { class: 'btn btn--primary', attrs: { type: 'submit' } }, 'Sign in');
    const form = h('form', { class: 'card login__card', on: { submit: (event) => {
      event.preventDefault();
      submit.disabled = true;
      error.textContent = '';
      command('sign_in', { username: user.value.trim(), password: pass.value })
        .then(() => resolve())
        .catch((e: unknown) => {
          const failure = failureFrom(e);
          error.textContent = failure.lockedUntil === null
            ? failure.message
            : `Too many failed attempts. Try again ${formatAge(failure.lockedUntil, Math.floor(Date.now() / 1000)).replace(' ago', '')} (until ${new Date(failure.lockedUntil * 1000).toLocaleTimeString()}).`;
          pass.value = '';
          submit.disabled = false;
        });
    } } },
      h('div', { class: 'login__brand' }, brandMark(), h('h1', {}, 'Kvasir'), h('p', { class: 'dim' }, 'Sign in with your Kvasir account')),
      h('div', { class: 'field' }, h('label', { attrs: { for: 'login-user' } }, 'Username'), user),
      h('div', { class: 'field' }, h('label', { attrs: { for: 'login-pass' } }, 'Password'), pass),
      error, submit);
    setChildren(root, h('main', { class: 'login' }, form));
    user.focus();
  });
}
