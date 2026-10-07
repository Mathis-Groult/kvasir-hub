import { h } from '../core/dom.ts';

const SVG_NS = 'http://www.w3.org/2000/svg';

/** Simple gold mark; the full logo stays on the website. */
export function brandMark(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', '0 0 32 32');
  svg.setAttribute('class', 'brand__mark');
  svg.setAttribute('aria-hidden', 'true');
  const shape = document.createElementNS(SVG_NS, 'path');
  shape.setAttribute('d', 'M16 2 28 9v14l-12 7L4 23V9z');
  shape.setAttribute('fill', 'var(--accent)');
  const letter = document.createElementNS(SVG_NS, 'path');
  letter.setAttribute('d', 'M12 9h2.6v5.2L19 9h3.2l-5.1 5.6L22.4 23h-3.1l-3.8-6.2-.9 1V23H12z');
  letter.setAttribute('fill', 'var(--on-accent)');
  svg.append(shape, letter);
  return svg;
}

export function brand(): HTMLElement {
  return h('div', { class: 'brand' }, brandMark(), 'Kvasir');
}
