// @ts-check
import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";

// https://astro.build/config
export default defineConfig({
  site: "https://closemylid.app",
  trailingSlash: "never",
  // Astro has no <Link> component; internal navigation is plain <a> plus its
  // built-in prefetcher, opted into per link with data-astro-prefetch.
  prefetch: true,
  integrations: [
    sitemap({
      // 404 is noindex, so it has no business in the sitemap.
      filter: (page) => !page.includes("/404"),
    }),
  ],
  vite: {
    plugins: [tailwindcss()]
  }
});
