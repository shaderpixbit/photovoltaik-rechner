<script lang="ts">
  import type { Component, Snippet } from "svelte";

  let {
    label,
    value,
    sub,
    icon,
    accent = "var(--tr-sun)",
    children,
  }: {
    label: string;
    value: string;
    sub?: string;
    icon?: Component;
    accent?: string;
    children?: Snippet;
  } = $props();
</script>

<div
  class="rounded-lg border border-[var(--tr-line)] bg-[var(--tr-surface)] px-5 py-4 shadow-sm"
>
  <div class="flex items-center justify-between">
    <span class="text-xs font-medium uppercase tracking-wide text-[var(--tr-text-dim)]">
      {label}
    </span>
    {#if icon}
      {@const Icon = icon}
      <Icon class="size-4" style="color: {accent};" />
    {/if}
  </div>
  <div class="mt-2 font-mono text-2xl font-semibold" style="color: {accent};">
    {value}
  </div>
  {#if sub}
    <div class="mt-1 text-xs text-[var(--tr-text-faint)]">{sub}</div>
  {/if}
  {@render children?.()}
</div>
