import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  base: '/bifrost/ui/',
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      '/bifrost': {
        target: 'http://192.168.2.6',
        changeOrigin: true,
      },
      '/eventstream': {
        target: 'http://192.168.2.6',
        changeOrigin: true,
      },
      '/clip': {
        target: 'http://192.168.2.6',
        changeOrigin: true,
      },
    },
  },
})
