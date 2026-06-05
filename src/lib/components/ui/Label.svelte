<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLLabelAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type Props = HTMLLabelAttributes & {
    children?: Snippet;
    class?: string;
  };

  let { class: className, children, ...rest }: Props = $props();
</script>

<label
  class={cn("text-xs font-medium text-[var(--tr-text-dim)]", className)}
  {...rest}
>
  {@render children?.()}
</label>
