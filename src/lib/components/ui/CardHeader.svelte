<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/utils";

  let {
    title,
    description,
    actions,
    class: className,
  }: {
    title: string;
    description?: string;
    actions?: Snippet;
    class?: string;
  } = $props();
</script>

<div
  class={cn(
    "flex items-start justify-between gap-4 border-b border-[var(--tr-line)] px-5 py-4",
    className,
  )}
>
  <div>
    <h2 class="text-base font-semibold leading-tight">{title}</h2>
    {#if description}
      <p class="mt-1 text-xs text-[var(--tr-text-dim)]">{description}</p>
    {/if}
  </div>
  {#if actions}
    <div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
  {/if}
</div>
