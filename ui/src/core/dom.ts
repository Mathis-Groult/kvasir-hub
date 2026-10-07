// Tiny element builder. Text always goes through textContent/createTextNode, never innerHTML,
// so server data can never become markup.

export type Child = Node | string | null | undefined | false;

type Handlers = {
  readonly [K in keyof HTMLElementEventMap]?: (event: HTMLElementEventMap[K]) => void;
};

export interface Props {
  readonly class?: string;
  /** Attributes set with setAttribute (href, type, aria-*, data-*, ...). */
  readonly attrs?: Readonly<Record<string, string>>;
  readonly on?: Handlers;
}

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Props = {},
  ...children: readonly Child[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  if (props.class !== undefined) element.className = props.class;
  for (const [name, value] of Object.entries(props.attrs ?? {})) element.setAttribute(name, value);
  for (const [name, handler] of Object.entries(props.on ?? {})) {
    // Object.entries widens the handler type; the Handlers mapping above guarantees the pairing.
    element.addEventListener(name, handler as EventListener);
  }
  append(element, children);
  return element;
}

export function append(parent: Element, children: readonly Child[]): void {
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    parent.append(typeof child === 'string' ? document.createTextNode(child) : child);
  }
}

/** Replaces every child of `parent`. */
export function setChildren(parent: Element, ...children: readonly Child[]): void {
  parent.replaceChildren();
  append(parent, children);
}

/** The element with `selector` inside `root`, or a loud error: a missing hook is a programmer bug. */
export function must<T extends Element>(root: ParentNode, selector: string, kind: new () => T): T {
  const found = root.querySelector(selector);
  if (!(found instanceof kind)) throw new Error(`missing element ${selector}`);
  return found;
}
