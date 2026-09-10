import { defineConfig } from 'vite';
// 此配置由 Node.js 读取，前端无需引入整套 Node.js 类型。
declare const process: { env: Record<string, string | undefined> };
const host = process.env.TAURI_DEV_HOST;
export default defineConfig({
  root: 'shared/ui',
  clearScreen: false,
  build: { outDir: '../../dist', emptyOutDir: true },
  server: {
    host: host || '127.0.0.1',
    port: 1420,
    strictPort: true,
    hmr: host ? { host, protocol: 'ws', port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
});
