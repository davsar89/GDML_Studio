import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// The store is a module-level singleton shared across every test in a file.
afterEach(() => {
  cleanup();
});
