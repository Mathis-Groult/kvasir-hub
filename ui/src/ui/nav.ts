// Which destinations the navigation shows. Pure, so the rule is testable.

export interface NavItem {
  readonly id: string;
  readonly label: string;
  readonly path: string;
}

/** Material 3 allows 3-5 bottom destinations: Home plus the first modules, the rest via Home. */
export const MAX_BOTTOM_ITEMS = 4;

export function bottomItems(all: readonly NavItem[]): readonly NavItem[] {
  return all.slice(0, MAX_BOTTOM_ITEMS);
}

/** The item whose path is a prefix of `current` (Home only matches exactly). */
export function activeItem(items: readonly NavItem[], current: string): NavItem | undefined {
  return items.find((item) => (item.path === '/' ? current === '/' : current === item.path || current.startsWith(`${item.path}/`)));
}
