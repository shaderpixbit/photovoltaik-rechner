<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { cn } from "$lib/utils";

  type Variant = "primary" | "secondary" | "ghost" | "danger";
  type Size = "sm" | "md" | "lg";

  type Props = HTMLButtonAttributes & {
    variant?: Variant;
    size?: Size;
    children?: Snippet;
    class?: string;
  };

  let {
    variant = "secondary",
    size = "md",
    class: className,
    type = "button",
    children,
    ...rest
  }: Props = $props();

  const variants: Record<Variant, string> = {
    primary:
      "bg-[var(--tr-sun)] text-black hover:bg-[var(--tr-sun)]/90 border-transparent",
    secondary:
      "bg-[var(--tr-surface)] hover:bg-[var(--tr-surface2)] border-[var(--tr-line)] text-[var(--tr-text)]",
    ghost:
      "bg-transparent hover:bg-[var(--tr-surface2)] border-transparent text-[var(--tr-text-dim)]",
    danger:
      "bg-[var(--tr-red)] text-white hover:bg-[var(--tr-red-dim)] border-transparent",
  };

  const sizes: Record<Size, string> = {
    sm: "h-8 px-3 text-xs",
    md: "h-9 px-4 text-sm",
    lg: "h-10 px-5 text-sm",
  };
</script>

<button
  {type}
  class={cn(
    "inline-flex items-center justify-center gap-1.5 rounded-md border font-medium transition-colors",
    "disabled:cursor-not-allowed disabled:opacity-50",
    variants[variant],
    sizes[size],
    className,
  )}
  {...rest}
>
  {@render children?.()}
</button>
