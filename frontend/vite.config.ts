/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:4001',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    // The default `forks` pool times out waiting for its worker to respond on
    // Windows; threads start reliably.
    pool: 'threads',
    // `globals: false` -- tests import describe/it/expect from 'vitest'
    // explicitly, so tsconfig.app.json needs no `types` entry and `tsc -b`
    // keeps type-checking the tests along with the rest of src.
    globals: false,
    setupFiles: ['./src/test/setup.ts'],
    // Viewport components need a WebGL context jsdom does not provide, so
    // only their pure three.js code (snapUtils, geometryCache) is tested here.
    include: ['src/**/*.test.{ts,tsx}'],
  },
})
