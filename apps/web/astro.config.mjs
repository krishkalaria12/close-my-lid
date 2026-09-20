// @ts-check
import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";

// https://astro.build/config
export default defineConfig({
  site: "https://closemylid.app",
  // Astro has no <Link> component; internal navigation is plain <a> plus its
  // built-in prefetcher, opted into per link with data-astro-prefetch.
  prefetch: true,
  vite: {
    plugins: [tailwindcss()]
  }
});
