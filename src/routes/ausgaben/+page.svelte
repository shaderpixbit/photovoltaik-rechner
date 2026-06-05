<script lang="ts">
  import { onMount } from "svelte";
  import { deleteExpense, listExpenses, upsertExpense } from "$lib/api";
  import type { Expense, ExpenseKategorie } from "$lib/types";
  import { formatDateDE, formatEUR, todayISO } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { PlusIcon, SaveIcon, Trash2Icon, XIcon } from "@lucide/svelte";

  const KATEGORIEN: ExpenseKategorie[] = [
    "Versicherung",
    "Wartung",
    "Reparatur",
    "Zaehlermiete",
    "Verwaltung",
    "Sonstiges",
  ];

  let items = $state<Expense[]>([]);
  let editing = $state<Expense | null>(null);
  let error = $state<string | null>(null);

  function leer(): Expense {
    return {
      id: 0,
      date: todayISO(),
      kategorie: "Versicherung",
      beschreibung: "",
      netto: 0,
      ust: 0,
      brutto: 0,
      vorsteuer_abzugsfaehig: true,
    };
  }

  async function reload() {
    try {
      items = await listExpenses();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(reload);

  async function save() {
    if (!editing) return;
    if (!editing.brutto) editing.brutto = (editing.netto ?? 0) + (editing.ust ?? 0);
    try {
      await upsertExpense(editing);
      editing = null;
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function remove(id: number) {
    if (!confirm("Eintrag löschen?")) return;
    await deleteExpense(id);
    await reload();
  }

  let summe = $derived(items.reduce((s, x) => s + x.netto, 0));
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Betriebsausgaben</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Versicherung, Wartung, Zählermiete u.ä. Anschaffungen → Anlage / AfA.
      </p>
    </div>
    <Button variant="primary" onclick={() => (editing = leer())}>
      <PlusIcon class="size-4" />Neue Ausgabe
    </Button>
  </div>

  {#if error}
    <Card><div class="p-5 text-sm text-[var(--tr-red)]">{error}</div></Card>
  {/if}

  {#if editing}
    <Card>
      <CardHeader
        title={editing.id ? `Ausgabe #${editing.id} bearbeiten` : "Neue Ausgabe"}
      />
      <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-4">
        <div class="space-y-1.5">
          <Label>Datum</Label>
          <Input type="date" bind:value={editing.date} />
        </div>
        <div class="space-y-1.5">
          <Label>Kategorie</Label>
          <Select
            bind:value={editing.kategorie}
            options={KATEGORIEN.map((k) => ({ value: k, label: k }))}
          />
        </div>
        <div class="space-y-1.5 md:col-span-2">
          <Label>Beschreibung</Label>
          <Input bind:value={editing.beschreibung} placeholder="z.B. Jahresprämie 2026" />
        </div>
        <div class="space-y-1.5">
          <Label>Netto (€)</Label>
          <Input type="number" step="0.01" bind:value={editing.netto} />
        </div>
        <div class="space-y-1.5">
          <Label>USt (€)</Label>
          <Input type="number" step="0.01" bind:value={editing.ust} />
        </div>
        <div class="space-y-1.5">
          <Label>Brutto (€)</Label>
          <Input type="number" step="0.01" bind:value={editing.brutto} />
        </div>
        <label
          class="flex items-end gap-2 text-sm text-[var(--tr-text-dim)]"
        >
          <input
            type="checkbox"
            class="size-4 rounded border-[var(--tr-line)]"
            bind:checked={editing.vorsteuer_abzugsfaehig}
          />
          Vorsteuerabzugsfähig
        </label>
        <div class="flex items-end gap-2 md:col-span-4">
          <Button variant="primary" onclick={save}>
            <SaveIcon class="size-4" />Speichern
          </Button>
          <Button variant="ghost" onclick={() => (editing = null)}>
            <XIcon class="size-4" />Abbrechen
          </Button>
        </div>
      </div>
    </Card>
  {/if}

  <Card>
    <CardHeader title="Alle Ausgaben" description={`Σ Netto: ${formatEUR(summe)}`} />
    {#if items.length === 0}
      <div class="px-5 py-6 text-sm text-[var(--tr-text-dim)]">Noch keine Einträge.</div>
    {:else}
      <table class="w-full text-sm">
        <thead
          class="bg-[var(--tr-surface2)] text-xs uppercase text-[var(--tr-text-dim)]"
        >
          <tr>
            <th class="px-5 py-2 text-left">Datum</th>
            <th class="px-5 py-2 text-left">Kategorie</th>
            <th class="px-5 py-2 text-left">Beschreibung</th>
            <th class="px-5 py-2 text-right">Netto</th>
            <th class="px-5 py-2 text-right">USt</th>
            <th class="px-5 py-2 text-right">Brutto</th>
            <th class="px-5 py-2 text-center">VSt</th>
            <th class="px-5 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each items as x (x.id)}
            <tr
              class="cursor-pointer border-t border-[var(--tr-line)] hover:bg-[var(--tr-surface2)]"
              onclick={() => (editing = { ...x })}
            >
              <td class="px-5 py-2 font-mono">{formatDateDE(x.date)}</td>
              <td class="px-5 py-2">{x.kategorie}</td>
              <td class="px-5 py-2 text-[var(--tr-text-dim)]">{x.beschreibung}</td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(x.netto)}</td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(x.ust)}</td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(x.brutto)}</td>
              <td class="px-5 py-2 text-center">
                {#if x.vorsteuer_abzugsfaehig}
                  <span style="color: var(--tr-green-dim);">✓</span>
                {:else}
                  <span class="text-[var(--tr-text-faint)]">—</span>
                {/if}
              </td>
              <td class="px-5 py-2 text-right">
                <button
                  type="button"
                  class="inline-flex h-7 w-7 items-center justify-center rounded-md text-[var(--tr-text-faint)] hover:bg-[var(--tr-red-bg)] hover:text-[var(--tr-red)]"
                  onclick={(e) => {
                    e.stopPropagation();
                    remove(x.id);
                  }}
                  aria-label="Löschen"
                >
                  <Trash2Icon class="size-4" />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </Card>
</div>
