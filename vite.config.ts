import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    open: false,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust build output
      ignored: ["**/src-tauri/**"],
    },
  },
});
