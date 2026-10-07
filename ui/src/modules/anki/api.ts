import { rec, str } from '../../core/decode.ts';
import { call } from '../../core/transport.ts';

export interface AnkiSetup {
  readonly syncUrl: string;
  /** Anki lower-cases the Kvasir name. */
  readonly username: string;
}

export const decodeSetup = (value: unknown): AnkiSetup => {
  const r = rec(value, 'setup');
  return { syncUrl: str(r, 'syncUrl'), username: str(r, 'username') };
};

export const ankiApi = {
  setup: async (): Promise<AnkiSetup> => decodeSetup(await call('anki', 'setup')),
};
