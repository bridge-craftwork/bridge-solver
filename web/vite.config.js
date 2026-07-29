import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],

  // Served from https://<org>.github.io/bridge-solver/, so assets must be
  // referenced relatively rather than from the domain root.
  base: './',

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  build: {
    outDir: 'dist',
    // The wasm module is a megabyte-ish on its own; the default 500 kB warning
    // is noise here rather than a signal.
    chunkSizeWarningLimit: 2000,
    target: 'es2022',
  },

  worker: {
    format: 'es',
  },

  // `wasm-pack --target web` output loads the binary with
  // `new URL('..._bg.wasm', import.meta.url)`, which Vite rewrites to a hashed
  // asset. Excluding it from dep optimisation keeps that URL intact in dev.
  optimizeDeps: {
    exclude: ['@/wasm/bridge_solver_wasm.js'],
  },

  test: {
    environment: 'node',
    include: ['src/**/*.test.js'],
  },
})
