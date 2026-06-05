<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type Props = HTMLAttributes<HTMLDivElement> & {
    children?: Snippet;
    class?: string;
  };

  let { class: className, children, ...rest }: Props = $props();
</script>

<div
  class={cn(
    "rounded-lg border bg-[var(--tr-surface)] shadow-sm",
    "border-[var(--tr-line)]",
    className,
  )}
  {...rest}
>
  {@render children?.()}
</div>
