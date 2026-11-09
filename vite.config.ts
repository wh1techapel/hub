import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri loads the built files from disk, so assets are relative. The dev port is fixed because
// src-tauri/tauri.conf.json points at it (ULTRON's shell uses 1420).
export default defineConfig({
  plugins: [react()],
  base: "./",
  clearScreen: false,
  server: { port: 1430, strictPort: true, host: "127.0.0.1" },
  // Two pages: the main window and the Catat Cepat window (Ctrl+Alt+H).
  build: {
    target: "es2023",
    sourcemap: false,
    chunkSizeWarningLimit: 600,
    rollupOptions: { input: { main: "index.html", capture: "capture.html" } },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    css: { modules: { classNameStrategy: "non-scoped" } },
  },
});
