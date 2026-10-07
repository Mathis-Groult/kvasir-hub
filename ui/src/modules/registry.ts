// How a module's UI plugs into the app. A module exports a `Page` (its routes); the app shell
// owns navigation, the player and error handling. Adding a module's UI = one entry in
// `modules/index.ts`.

import type { IconName } from '../core/icons.ts';
import type { Location, Params } from '../core/router.ts';
import type { Player } from '../ui/player.ts';

export interface PageContext {
  readonly params: Params;
  readonly location: Location;
  readonly player: Player;
}

export interface Route {
  /** `/files/:repo` style pattern, absolute. */
  readonly pattern: string;
  readonly render: (context: PageContext) => HTMLElement;
}

export interface Page {
  readonly routes: readonly Route[];
}

/** UI-side description of a module; the title and summary come from the Rust manifest. */
export interface ModuleUi {
  readonly id: string;
  readonly icon: IconName;
  /** Where the module's home page lives. */
  readonly path: string;
  readonly page: Page;
  /** Optional live summary shown on Home. */
  readonly widget?: () => Promise<HTMLElement | null>;
}
