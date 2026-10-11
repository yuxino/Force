import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: { port: 3415, strictPort: true, proxy: { '/dev-api': {
    target: 'http://127.0.0.1:3459', rewrite: path => path.replace(/^\/dev-api/, ''),
  } } },
  build: { target: 'safari15' },
});
