// The loading / error / retry cycle every list screen needs, written once.

import { setChildren } from '../core/dom.ts';
import { failureFrom, type Failure } from '../core/transport.ts';
import { button, stateBlock } from './components.ts';

export interface LoadOptions<T> {
  readonly placeholder: () => HTMLElement;
  readonly load: () => Promise<T>;
  readonly render: (data: T) => HTMLElement | readonly HTMLElement[];
}

/** Shows `placeholder`, then the rendered data, or an error block with Retry. */
export async function loadInto<T>(container: HTMLElement, options: LoadOptions<T>): Promise<void> {
  setChildren(container, options.placeholder());
  try {
    const data = await options.load();
    const rendered = options.render(data);
    setChildren(container, ...(Array.isArray(rendered) ? rendered : [rendered]));
  } catch (error) {
    const failure: Failure = failureFrom(error);
    const retry = failure.retryable || failure.code === 'internal' || failure.code === 'malformed';
    setChildren(container, stateBlock('warning', 'Something went wrong', failure.message,
      retry ? button('Try again', () => { void loadInto(container, options); }, 'primary', 'refresh') : undefined, true));
  }
}
