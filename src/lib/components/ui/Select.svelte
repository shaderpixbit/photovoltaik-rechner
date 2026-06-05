<script lang="ts">
  import type { HTMLSelectAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type Option = { value: string; label: string };

  type Props = HTMLSelectAttributes & {
    value?: string;
    options: Option[];
    class?: string;
  };

  let { value = $bindable(), options, class: className, ...rest }: Props = $props();
</script>

<select
  bind:value
  class={cn(
    "h-9 w-full rounded-md border bg-[var(--tr-surface)] px-3 text-sm",
    "border-[var(--tr-line)] focus:border-[var(--tr-sun)] focus:outline-none",
    className,
  )}
  {...rest}
>
  {#each options as opt (opt.value)}
    <option value={opt.value}>{opt.label}</option>
  {/each}
</select>
