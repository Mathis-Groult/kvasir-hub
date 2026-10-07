// The ONLY file that knows how the UI reaches the Rust core: Tauri `invoke` inside the app,
// plain HTTP against `hub-web` in development and tests. Everything else calls `call()`.

import { DecodeError, bool, rec, str, type Rec } from './decode.ts';

export interface Failure {
  readonly code: string;
  readonly message: string;
  readonly retryable: boolean;
  /** Unix seconds; present for `locked`. */
  readonly lockedUntil: number | null;
}

/** A failure answered by the core (as opposed to a bug in this UI). */
export class HubRequestError extends Error {
  readonly failure: Failure;
  constructor(failure: Failure) {
    super(failure.message);
    this.name = 'HubRequestError';
    this.failure = failure;
  }
}

interface Backend {
  rpc(command: string, args: unknown): Promise<unknown>;
  upload(target: string, body: ArrayBuffer): Promise<unknown>;
  /** Origin that serves `/<module>/<op>?..` media. */
  readonly mediaBase: string;
}

interface TauriInternals {
  invoke(command: string, args?: unknown, options?: { headers: Record<string, string> }): Promise<unknown>;
}

function tauriInternals(): TauriInternals | null {
  const candidate: unknown = Reflect.get(window, '__TAURI_INTERNALS__');
  if (typeof candidate !== 'object' || candidate === null) return null;
  return typeof Reflect.get(candidate, 'invoke') === 'function' ? (candidate as TauriInternals) : null;
}

/** Tauri serves custom schemes as `scheme://localhost` except on Windows and Android. */
function tauriMediaBase(): string {
  return /Android|Windows/.test(navigator.userAgent) ? 'http://hubmedia.localhost' : 'hubmedia://localhost';
}

function webBackend(): Backend {
  return {
    mediaBase: '/media',
    async rpc(command, args) {
      const response = await fetch(`/rpc/${command}`, { method: 'POST', body: JSON.stringify(args) });
      return response.json();
    },
    async upload(target, body) {
      const response = await fetch(`/upload${target}`, { method: 'POST', body });
      return response.json();
    },
  };
}

function tauriBackend(internals: TauriInternals): Backend {
  return {
    mediaBase: tauriMediaBase(),
    rpc: (command, args) => internals.invoke('hub_rpc', { command, args }),
    upload: (target, body) => internals.invoke('hub_upload', body, { headers: { 'x-hub-target': target } }),
  };
}

// Chosen on first use, not at import time: pure modules (decoders) import this file and must
// load without a browser.
let chosen: Backend | null = null;

function backend(): Backend {
  if (chosen === null) {
    const internals = tauriInternals();
    chosen = internals === null ? webBackend() : tauriBackend(internals);
  }
  return chosen;
}

/** Reads `{ok:true,data}` or `{ok:false,error}`; throws `HubRequestError` for the latter. */
export function unwrap(envelope: unknown): unknown {
  const body = rec(envelope, 'answer');
  if (bool(body, 'ok')) return body['data'];
  const error = rec(body['error'], 'error');
  const lockedUntil = error['locked_until'];
  throw new HubRequestError({
    code: str(error, 'code'),
    message: str(error, 'message'),
    retryable: bool(error, 'retryable'),
    lockedUntil: typeof lockedUntil === 'number' ? lockedUntil : null,
  });
}

/** A transport-level problem (the core could not even be reached) as a Failure. */
export function failureFrom(error: unknown): Failure {
  if (error instanceof HubRequestError) return error.failure;
  const message = error instanceof Error ? error.message : 'Unexpected error';
  const kind = error instanceof DecodeError ? 'malformed' : 'internal';
  return { code: kind, message, retryable: false, lockedUntil: null };
}

export async function command(name: string, args: Rec = {}): Promise<unknown> {
  return unwrap(await backend().rpc(name, args));
}

/** Runs `op` of `module` and returns the raw JSON; callers decode it. */
export function call(module: string, op: string, args: Rec = {}): Promise<unknown> {
  return command('call', { module, op, args });
}

/** Upload raw bytes to a module operation. `params` become the operation's arguments. */
export async function upload(
  module: string,
  op: string,
  params: Readonly<Record<string, string>>,
  body: ArrayBuffer,
): Promise<unknown> {
  const query = new URLSearchParams(params).toString();
  return unwrap(await backend().upload(`/${module}/${op}?${query}`, body));
}

/** URL the webview can load for a media operation (audio, covers, downloads). */
export function mediaUrl(module: string, op: string, params: Readonly<Record<string, string>>): string {
  const query = new URLSearchParams(params).toString();
  return `${backend().mediaBase}/${module}/${op}?${query}`;
}
