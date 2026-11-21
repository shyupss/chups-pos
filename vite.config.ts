import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Configuración pensada para Tauri: puerto fijo y sin limpiar la consola,
// para ver los errores de Rust junto a los de Vite.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**"] },
  },
  build: {
    // WebView2 (Windows 10) es Chromium moderno: no hace falta transpilar de más.
    target: "chrome105",
    minify: "esbuild",
    sourcemap: false,
  },
});
