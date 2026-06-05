<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type Props = HTMLInputAttributes & {
    value?: string | number | null;
    class?: string;
  };

  let { value = $bindable(), class: className, ...rest }: Props = $props();
</script>

<input
  bind:value
  class={cn(
    "h-9 w-full rounded-md border bg-[var(--tr-surface)] px-3 text-sm",
    "border-[var(--tr-line)] focus:border-[var(--tr-sun)] focus:outline-none",
    "placeholder:text-[var(--tr-text-faint)]",
    className,
  )}
  {...rest}
/>
