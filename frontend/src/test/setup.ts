import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// jsdom has no scrolling layout; keyboard list navigation calls the browser API.
Element.prototype.scrollIntoView = () => {};

// jsdom ships no ResizeObserver. It is baseline in every browser the app
// targets, so stub it here rather than making production code defend against
// its absence. Nothing lays out in jsdom, so a no-op is a faithful stand-in.
if (!('ResizeObserver' in globalThis)) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}

// With `globals: false` Testing Library finds no global afterEach to register its own cleanup.
afterEach(() => {
  cleanup();
});
