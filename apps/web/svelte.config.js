import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// TypeScript in <script lang="ts">; read by Vite and by svelte-check
export default {
    preprocess: vitePreprocess(),
};
