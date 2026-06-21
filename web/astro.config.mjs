import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://desktopfushi.mercen.net",
  output: "static",
  vite: {
    build: {
      assetsInlineLimit: 0,
    },
  },
});
