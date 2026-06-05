<script lang="ts">
  import { onMount } from "svelte";
  import {
    deletePayout,
    getExpectedEinspeisung,
    listPayouts,
    upsertPayout,
  } from "$lib/api";
  import type { ExpectedEinspeisung, Payout } from "$lib/types";
  import { formatDateDE, formatEUR, todayISO } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { PlusIcon, SaveIcon, Trash2Icon, XIcon } from "@lucide/svelte";

  let payouts = $state<Payout[]>([]);
  let editing = $state<Payout | null>(null);
  let error = $state<string | null>(null);

  const currentYear = new Date().getFullYear();
  let jahrSel = $state(String(currentYear));
  let jahr = $derived(Number(jahrSel));
  const jahre = Array.from({ length: 10 }, (_, i) => currentYear - i);
  let expected = $state<ExpectedEinspeisung | null>(null);

  function leer(): Payout {
    return {
      id: 0,
      buchung_date: todayISO(),
      zeitraum_von: "",
      zeitraum_bis: "",
      netto: 0,
      ust: 0,
      brutto: 0,
      kwh: null,
      notiz: null,
    };
  }

  async function reload() {
    try {
      payouts = await listPayouts();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function reloadExpected() {
    try {
      expected = await getExpectedEinspeisung(jahr, null);
    } catch (e) {
      expected = null;
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(async () => {
    await reload();
    await reloadExpected();
  });

  $effect(() => {
    jahrSel;
    reloadExpected();
  });

  async function save() {
    if (!editing) return;
    try {
      // Brutto auto-berechnen falls leer / 0
      if (!editing.brutto || editing.brutto === 0) {
        editing.brutto = (editing.netto ?? 0) + (editing.ust ?? 0);
      }
      await upsertPayout(editing);
      editing = null;
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function remove(id: number) {
    if (!confirm("Auszahlung wirklich löschen?")) return;
    try {
      await deletePayout(id);
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  let summeNetto = $derived(payouts.reduce((s, p) => s + p.netto, 0));
  let summeBrutto = $derived(payouts.reduce((s, p) => s + p.brutto, 0));

  let payoutsJahr = $derived(
    payouts.filter((p) => p.buchung_date.startsWith(String(jahr))),
  );
  let istNetto = $derived(payoutsJahr.reduce((s, p) => s + p.netto, 0));
  let istKwh = $derived(payoutsJahr.reduce((s, p) => s + (p.kwh ?? 0), 0));
  let diffNetto = $derived(expected ? istNetto - expected.erwartet_netto : 0);
  let diffPct = $derived(
    expected && expected.erwartet_netto > 0
      ? (diffNetto / expected.erwartet_netto) * 100
      : 0,
  );
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Bayernwerk-Auszahlungen</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Eine Buchung pro Abrechnung. Netto + USt = Brutto.
      </p>
    </div>
    <Button variant="primary" onclick={() => (editing = leer())}>
      <PlusIcon class="size-4" />
      Neue Buchung
    </Button>
  </div>

  {#if error}
    <Card>
      <div class="p-5 text-sm text-[var(--tr-red)]">{error}</div>
    </Card>
  {/if}

  <Card>
    <CardHeader
      title="Soll / Ist — Einspeisevergütung"
      description="Erwartete Vergütung aus Tageserzeugung × Verlauf-Satz vs. tatsächlich vereinnahmt."
    />
    <div class="flex items-end gap-4 px-5 pt-5">
      <div class="w-32 space-y-1.5">
        <Label>Jahr</Label>
        <Select
          bind:value={jahrSel}
          options={jahre.map((y) => ({ value: String(y), label: String(y) }))}
        />
      </div>
    </div>
    <dl class="grid grid-cols-2 divide-x divide-[var(--tr-line)] px-0 pt-4 md:grid-cols-4">
      <div class="px-5 py-4">
        <dt class="text-xs uppercase text-[var(--tr-text-dim)]">Erwartet (netto)</dt>
        <dd class="mt-1 font-mono text-lg">
          {expected ? formatEUR(expected.erwartet_netto) : "—"}
        </dd>
        <div class="mt-0.5 text-xs text-[var(--tr-text-faint)]">
          {expected
            ? `aus ${expected.kwh.toLocaleString("de-DE", { maximumFractionDigits: 0 })} kWh`
            : ""}
        </div>
      </div>
      <div class="px-5 py-4">
        <dt class="text-xs uppercase text-[var(--tr-text-dim)]">Tatsächlich (netto)</dt>
        <dd class="mt-1 font-mono text-lg">{formatEUR(istNetto)}</dd>
        <div class="mt-0.5 text-xs text-[var(--tr-text-faint)]">
          {istKwh > 0
            ? `aus ${istKwh.toLocaleString("de-DE", { maximumFractionDigits: 0 })} kWh laut Abrechnung`
            : `${payoutsJahr.length} Buchung(en)`}
        </div>
      </div>
      <div class="px-5 py-4">
        <dt class="text-xs uppercase text-[var(--tr-text-dim)]">Differenz</dt>
        <dd
          class="mt-1 font-mono text-lg"
          style:color={Math.abs(diffNetto) < 0.5
            ? "var(--tr-text)"
            : diffNetto > 0
              ? "var(--tr-green-dim)"
              : "var(--tr-red)"}
        >
          {expected ? formatEUR(diffNetto) : "—"}
        </dd>
        <div class="mt-0.5 text-xs text-[var(--tr-text-faint)]">
          {expected && expected.erwartet_netto > 0
            ? `${diffNetto >= 0 ? "+" : ""}${diffPct.toLocaleString("de-DE", { maximumFractionDigits: 1 })}%`
            : ""}
        </div>
      </div>
      <div class="px-5 py-4">
        <dt class="text-xs uppercase text-[var(--tr-text-dim)]">Hinweis</dt>
        <dd class="mt-1 text-sm text-[var(--tr-text-dim)]">
          {#if !expected || expected.kwh === 0}
            Keine Einspeisedaten für {jahr}.
          {:else if expected.tage_ohne_satz > 0}
            <span style="color: var(--tr-warning);">
              {expected.tage_ohne_satz} Tag(e) ohne Vergütungssatz —
              Verlauf in Einstellungen ergänzen.
            </span>
          {:else if expected.erwartet_netto === 0}
            Kein Vergütungssatz hinterlegt.
          {:else}
            Vergleich basiert auf Buchungsdatum.
          {/if}
        </dd>
      </div>
    </dl>
  </Card>

  {#if editing}
    <Card>
      <CardHeader
        title={editing.id ? `Buchung #${editing.id} bearbeiten` : "Neue Auszahlung"}
        description="Felder gemäß Bayernwerk-Gutschriftsanzeige."
      />
      <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-4">
        <div class="space-y-1.5">
          <Label>Buchungsdatum</Label>
          <Input type="date" bind:value={editing.buchung_date} />
        </div>
        <div class="space-y-1.5">
          <Label>Zeitraum von</Label>
          <Input type="date" bind:value={editing.zeitraum_von} />
        </div>
        <div class="space-y-1.5">
          <Label>Zeitraum bis</Label>
          <Input type="date" bind:value={editing.zeitraum_bis} />
        </div>
        <div class="space-y-1.5">
          <Label>kWh (laut Abrechnung)</Label>
          <Input
            type="number"
            step="0.1"
            min="0"
            value={editing.kwh ?? ""}
            oninput={(e) => {
              const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
              if (editing) editing.kwh = Number.isNaN(v) ? null : v;
            }}
          />
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
    <CardHeader
      title="Alle Auszahlungen"
      description={`Σ Netto: ${formatEUR(summeNetto)} · Σ Brutto: ${formatEUR(summeBrutto)}`}
    />
    {#if payouts.length === 0}
      <div class="px-5 py-6 text-sm text-[var(--tr-text-dim)]">
        Noch keine Auszahlungen erfasst.
      </div>
    {:else}
      <table class="w-full text-sm">
        <thead
          class="bg-[var(--tr-surface2)] text-xs uppercase text-[var(--tr-text-dim)]"
        >
          <tr>
            <th class="px-5 py-2 text-left">Buchung</th>
            <th class="px-5 py-2 text-left">Zeitraum</th>
            <th class="px-5 py-2 text-right">kWh</th>
            <th class="px-5 py-2 text-right">Netto</th>
            <th class="px-5 py-2 text-right">USt</th>
            <th class="px-5 py-2 text-right">Brutto</th>
            <th class="px-5 py-2 text-left">Notiz</th>
            <th class="px-5 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each payouts as p (p.id)}
            <tr
              class="cursor-pointer border-t border-[var(--tr-line)] hover:bg-[var(--tr-surface2)]"
              onclick={() => (editing = { ...p })}
            >
              <td class="px-5 py-2 font-mono">{formatDateDE(p.buchung_date)}</td>
              <td class="px-5 py-2 text-[var(--tr-text-dim)]">
                {formatDateDE(p.zeitraum_von)} – {formatDateDE(p.zeitraum_bis)}
              </td>
              <td class="px-5 py-2 text-right font-mono">
                {p.kwh ? p.kwh.toLocaleString("de-DE") : "—"}
              </td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(p.netto)}</td>
              <td class="px-5 py-2 text-right font-mono">{formatEUR(p.ust)}</td>
              <td class="px-5 py-2 text-right font-mono font-semibold">
                {formatEUR(p.brutto)}
              </td>
              <td class="px-5 py-2 text-[var(--tr-text-dim)]">{p.notiz ?? ""}</td>
              <td class="px-5 py-2 text-right">
                <button
                  type="button"
                  class="inline-flex h-7 w-7 items-center justify-center rounded-md text-[var(--tr-text-faint)] hover:bg-[var(--tr-red-bg)] hover:text-[var(--tr-red)]"
                  onclick={(e) => {
                    e.stopPropagation();
                    remove(p.id);
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
