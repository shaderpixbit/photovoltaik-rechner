<script lang="ts">
  import { onMount } from "svelte";
  import { deleteAsset, listAssets, upsertAsset } from "$lib/api";
  import type { Asset } from "$lib/types";
  import { formatDateDE, formatEUR, todayISO } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import { PlusIcon, SaveIcon, Trash2Icon, XIcon } from "@lucide/svelte";

  let assets = $state<Asset[]>([]);
  let editing = $state<Asset | null>(null);
  let error = $state<string | null>(null);

  function leer(): Asset {
    return {
      id: 0,
      name: "PV-Anlage",
      inbetriebnahme: todayISO(),
      anschaffung_netto: 0,
      anschaffung_ust: 0,
      nutzungsdauer_jahre: 20,
      notiz: null,
    };
  }

  async function reload() {
    try {
      assets = await listAssets();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
  onMount(reload);

  async function save() {
    if (!editing) return;
    try {
      await upsertAsset(editing);
      editing = null;
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
  async function remove(id: number) {
    if (!confirm("Anlage löschen?")) return;
    await deleteAsset(id);
    await reload();
  }

  function afaProJahr(a: Asset): number {
    const basis = a.anschaffung_netto + a.anschaffung_ust;
    return basis / Math.max(1, a.nutzungsdauer_jahre);
  }

  function abgeschriebenBis(a: Asset, heute: string): number {
    const start = new Date(a.inbetriebnahme);
    const now = new Date(heute);
    if (now < start) return 0;
    const monate =
      (now.getFullYear() - start.getFullYear()) * 12 +
      (now.getMonth() - start.getMonth()) +
      1;
    const proMonat = afaProJahr(a) / 12;
    const wert = monate * proMonat;
    const basis = a.anschaffung_netto + a.anschaffung_ust;
    return Math.min(wert, basis);
  }

  /**
   * EEG-Förderzeitraum endet am 31.12. des 20. Folgejahres nach Inbetriebnahme.
   * Beispiel: Inbetriebnahme 2024-06-01 → Förderung bis 2044-12-31.
   */
  function eegEnde(inbetriebnahme: string): Date | null {
    const start = new Date(inbetriebnahme);
    if (Number.isNaN(start.getTime())) return null;
    return new Date(start.getFullYear() + 20, 11, 31);
  }

  function eegRestText(inbetriebnahme: string, heute: string): string {
    const ende = eegEnde(inbetriebnahme);
    if (!ende) return "—";
    const now = new Date(heute);
    if (now > ende) return "abgelaufen";
    const totalMonths =
      (ende.getFullYear() - now.getFullYear()) * 12 +
      (ende.getMonth() - now.getMonth());
    const jahre = Math.floor(totalMonths / 12);
    const monate = totalMonths % 12;
    if (jahre <= 0) return `${monate} Mon.`;
    if (monate === 0) return `${jahre} J.`;
    return `${jahre} J. ${monate} Mon.`;
  }

  function eegEndeText(inbetriebnahme: string): string {
    const ende = eegEnde(inbetriebnahme);
    if (!ende) return "—";
    return ende.toLocaleDateString("de-DE", { month: "2-digit", year: "numeric" });
  }

  const today = todayISO();
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Anlage / AfA</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Anschaffungskosten + Nutzungsdauer → lineare AfA (Standard 20 Jahre).
      </p>
    </div>
    <Button variant="primary" onclick={() => (editing = leer())}>
      <PlusIcon class="size-4" />Neue Anlage
    </Button>
  </div>

  {#if error}
    <Card><div class="p-5 text-sm text-[var(--tr-red)]">{error}</div></Card>
  {/if}

  {#if editing}
    <Card>
      <CardHeader title={editing.id ? "Anlage bearbeiten" : "Neue Anlage"} />
      <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-3">
        <div class="space-y-1.5 md:col-span-2">
          <Label>Bezeichnung</Label>
          <Input bind:value={editing.name} placeholder="PV-Anlage Haus" />
        </div>
        <div class="space-y-1.5">
          <Label>Inbetriebnahme</Label>
          <Input type="date" bind:value={editing.inbetriebnahme} />
        </div>
        <div class="space-y-1.5">
          <Label>Anschaffung Netto (€)</Label>
          <Input type="number" step="0.01" bind:value={editing.anschaffung_netto} />
        </div>
        <div class="space-y-1.5">
          <Label>Anschaffung USt (€)</Label>
          <Input type="number" step="0.01" bind:value={editing.anschaffung_ust} />
        </div>
        <div class="space-y-1.5">
          <Label>Nutzungsdauer (Jahre)</Label>
          <Input type="number" min="1" bind:value={editing.nutzungsdauer_jahre} />
        </div>
        <div class="space-y-1.5 md:col-span-3">
          <Label>Notiz</Label>
          <Input
            value={editing.notiz ?? ""}
            oninput={(e) => {
              if (editing)
                editing.notiz = (e.currentTarget as HTMLInputElement).value || null;
            }}
          />
        </div>
        <div class="flex items-end gap-2 md:col-span-3">
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
    <CardHeader
      title="Anlagenverzeichnis"
      description="Lineare Abschreibung auf Brutto-Anschaffungskosten."
    />
    {#if assets.length === 0}
      <div class="px-5 py-6 text-sm text-[var(--tr-text-dim)]">
        Noch keine Anlagen erfasst.
      </div>
    {:else}
      <table class="w-full text-sm">
        <thead
          class="bg-[var(--tr-surface2)] text-xs uppercase text-[var(--tr-text-dim)]"
        >
          <tr>
            <th class="px-5 py-2 text-left">Anlage</th>
            <th class="px-5 py-2 text-left">Inbetriebnahme</th>
            <th class="px-5 py-2 text-right">Anschaff. brutto</th>
            <th class="px-5 py-2 text-right">ND</th>
            <th class="px-5 py-2 text-right">AfA / Jahr</th>
            <th class="px-5 py-2 text-right">Bereits abgeschr.</th>
            <th class="px-5 py-2 text-right">EEG bis</th>
            <th class="px-5 py-2 text-right">Rest</th>
            <th class="px-5 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each assets as a (a.id)}
            {@const basis = a.anschaffung_netto + a.anschaffung_ust}
            <tr
              class="cursor-pointer border-t border-[var(--tr-line)] hover:bg-[var(--tr-surface2)]"
              onclick={() => (editing = { ...a })}
            >
              <td class="px-5 py-2 font-medium">{a.name}</td>
              <td class="px-5 py-2 font-mono">{formatDateDE(a.inbetriebnahme)}</td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(basis)}</td>
              <td class="px-5 py-2 text-right font-mono">{a.nutzungsdauer_jahre} J</td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(afaProJahr(a))}</td>
              <td class="px-5 py-2 text-right font-mono">
                {formatEUR(abgeschriebenBis(a, today))}
              </td>
              <td class="px-5 py-2 text-right font-mono text-[var(--tr-text-dim)]">
                {eegEndeText(a.inbetriebnahme)}
              </td>
              <td class="px-5 py-2 text-right font-mono text-[var(--tr-text-dim)]">
                {eegRestText(a.inbetriebnahme, today)}
              </td>
              <td class="px-5 py-2 text-right">
                <button
                  type="button"
                  class="inline-flex h-7 w-7 items-center justify-center rounded-md text-[var(--tr-text-faint)] hover:bg-[var(--tr-red-bg)] hover:text-[var(--tr-red)]"
                  onclick={(e) => {
                    e.stopPropagation();
                    remove(a.id);
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
