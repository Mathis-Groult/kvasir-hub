// THE UI side of the module catalog. Adding an app's page = one entry here (plus its folder
// under modules/ and the Rust crate registered in hub-catalog).

import { ankiPageDef } from './anki/page.ts';
import { filesPage, filesWidget } from './files/page.ts';
import { musicPage } from './music/page.ts';
import { musicWidget } from './music/widget.ts';
import type { ModuleUi } from './registry.ts';

export const MODULES: readonly ModuleUi[] = [
  { id: 'files', icon: 'folder', path: '/files', page: filesPage, widget: filesWidget },
  { id: 'music', icon: 'music', path: '/music', page: musicPage, widget: musicWidget },
  { id: 'anki', icon: 'refresh', path: '/anki', page: ankiPageDef },
];
