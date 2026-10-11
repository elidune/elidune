import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

const useHttps = process.env.VITE_HTTPS === 'true'

// https://vite.dev/config/
export default defineConfig(async () => {
  const plugins = [react(), tailwindcss()]
  if (useHttps) {
    const { default: basicSsl } = await import('@vitejs/plugin-basic-ssl')
    plugins.push(basicSsl())
  }

  return {
    plugins,
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    server: {
      host: true,
      port: 3000,
      fs: {
        // The country zone table lives in server/data, outside this package.
        allow: [path.resolve(__dirname, '.'), path.resolve(__dirname, '../server/data')],
      },
      proxy: {
        '/api': {
          target: 'http://127.0.0.1:8080',
          changeOrigin: true,
        },
      },
    },
  }
})
