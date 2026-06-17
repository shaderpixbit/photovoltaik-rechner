import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [tailwindcss(), sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // Vite respektiert .gitignore nicht — schwere Verzeichnisse explizit
      // ausschliessen, sonst sprengt der File-Watcher das System-inotify-Limit.
      // .direnv/flake-inputs/ enthaelt eine komplette Kopie von nixpkgs.
      ignored: [
        "**/src-tauri/**",
        "**/.direnv/**",
        "**/.svelte-kit/**",
        "**/build/**",
        "**/vendor-import-anker/.venv/**",
        "**/vendor-import-anker/.pyinstaller-build/**",
      ],
    },
  },
}));
