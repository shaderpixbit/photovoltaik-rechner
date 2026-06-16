<script
  lang="ts"
  generics="T extends { id: number; effective_from: string }"
>
  import type { Snippet } from "svelte";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import DateField from "$lib/components/ui/DateField.svelte";
  import { PlusIcon, Trash2Icon } from "@lucide/svelte";

  type Props = {
    title: string;
    description?: string;
    items: T[];
    onAdd: () => void;
    onRemove: (id: number) => void;
    /** Anzahl Spalten im Row-Grid (md+). Default 3 — DateField + 1 Feld + Remove. */
    columns?: 3 | 4;
    /** Hinweis bei leerer Liste. Wenn null, wird nichts angezeigt. */
    emptyMessage?: string | null;
    /** Snippet für die Zellen *zwischen* "Gültig ab" und "Entfernen". */
    row: Snippet<[T]>;
  };

  let {
    title,
    description,
    items,
    onAdd,
    onRemove,
    columns = 3,
    emptyMessage = null,
    row,
  }: Props = $props();

  const gridClass = $derived(columns === 4 ? "md:grid-cols-4" : "md:grid-cols-3");
</script>

<Card>
  <CardHeader {title} {description} />
  <div class="divide-y divide-[var(--tr-line)]">
    {#each items as p (p.id)}
      <div class="grid grid-cols-1 items-end gap-3 px-5 py-3 {gridClass}">
        <div class="space-y-1.5">
          <Label>Gültig ab</Label>
          <DateField bind:value={p.effective_from} />
        </div>
        {@render row(p)}
        <div class="flex items-end">
          <Button variant="ghost" onclick={() => onRemove(p.id)}>
            <Trash2Icon class="size-4" />Entfernen
          </Button>
        </div>
      </div>
    {:else}
      {#if emptyMessage}
        <div class="px-5 py-3 text-xs text-[var(--tr-text-dim)]">
          {emptyMessage}
        </div>
      {/if}
    {/each}
  </div>
  <div class="border-t border-[var(--tr-line)] px-5 py-3">
    <Button variant="secondary" onclick={onAdd}>
      <PlusIcon class="size-4" />Periode hinzufügen
    </Button>
  </div>
</Card>
