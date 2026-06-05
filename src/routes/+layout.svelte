<script lang="ts">
  import "../app.css";
  import { ModeWatcher } from "mode-watcher";
  import Nav from "$lib/components/Nav.svelte";

  const { children } = $props();
</script>

<ModeWatcher />

<div class="min-h-screen bg-[var(--tr-bg)] text-[var(--tr-text)]">
  <Nav />
  <main class="mx-auto max-w-[1600px] px-6 py-6">
    {@render children()}
  </main>
</div>
