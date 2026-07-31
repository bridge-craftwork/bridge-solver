import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const entry = (name) => fileURLToPath(new URL(name, import.meta.url))

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
    // Two pages: the app, and a gallery that embeds it at each viewport. The
    // gallery imports the shared example module, so it has to be an entry rather
    // than a static file copied past the bundler.
    rollupOptions: {
      input: {
        main: entry('index.html'),
        gallery: entry('gallery.html'),
      },
    },
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
