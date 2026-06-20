<script lang="ts">
  import { onMount } from "svelte";
  import { aggregate } from "$lib/api";
  import type { Aggregat, Periode } from "$lib/types";
  import { formatKWh, formatPct } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Label from "$lib/components/ui/Label.svelte";

  const currentYear = new Date().getFullYear();
  let periodeSel = $state<string>("monat");
  let periode = $derived(periodeSel as Periode);
  let jahrSel = $state<string>(String(currentYear));
  let rows = $state<Aggregat[]>([]);
  let error = $state<string | null>(null);

  const jahre = Array.from({ length: 10 }, (_, i) => currentYear - i);

  async function load() {
    error = null;
    try {
      const j =
        jahrSel === "alle" || periode === "jahr" || periode === "max"
          ? null
          : Number(jahrSel);
      rows = await aggregate(periode, j);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);
  $effect(() => {
    periodeSel;
    jahrSel;
    load();
  });

  let maxErz = $derived(rows.reduce((m, r) => Math.max(m, r.erzeugung_kwh), 0));
  let totals = $derived(
    rows.reduce(
      (acc, r) => ({
        erz: acc.erz + r.erzeugung_kwh,
        ev: acc.ev + r.eigenverbrauch_kwh,
        ei: acc.ei + r.einspeisung_kwh,
        nb: acc.nb + r.netzbezug_kwh,
        tage: acc.tage + r.tage,
      }),
      { erz: 0, ev: 0, ei: 0, nb: 0, tage: 0 },
    ),
  );

  function bucketLabel(b: string): string {
    if (periode === "tag") {
      const [y, m, d] = b.split("-");
      return `${d}.${m}.${y.slice(2)}`;
    }
    if (periode === "monat") {
      const [y, m] = b.split("-");
      const names = ["Jan","Feb","Mrz","Apr","Mai","Jun","Jul","Aug","Sep","Okt","Nov","Dez"];
      return `${names[Number(m) - 1]} ${y}`;
    }
    if (periode === "jahr") return b;
    return "Gesamt";
  }
</script>

<div class="space-y-6">
  <div class="flex flex-wrap items-end justify-between gap-4">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Statistik</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Wähle Granularität — Max-Werte, Jahresvergleich, Monate, Tage.
      </p>
    </div>
    <div class="flex flex-wrap items-end gap-3">
      <div class="w-40 space-y-1.5">
        <Label>Periode</Label>
        <Select
          bind:value={periodeSel}
          options={[
            { value: "tag", label: "Tag" },
            { value: "monat", label: "Monat" },
            { value: "jahr", label: "Jahr" },
            { value: "max", label: "Max (Gesamt)" },
          ]}
        />
      </div>
      {#if periode === "tag" || periode === "monat"}
        <div class="w-32 space-y-1.5">
          <Label>Jahr</Label>
          <Select
            bind:value={jahrSel}
            options={[
              { value: "alle", label: "Alle" },
              ...jahre.map((y) => ({ value: String(y), label: String(y) })),
            ]}
          />
        </div>
      {/if}
    </div>
  </div>

  {#if error}
    <Card><div class="p-5 text-sm text-[var(--tr-red)]">{error}</div></Card>
  {/if}

  <div class="grid grid-cols-2 gap-4 lg:grid-cols-5">
    <Card>
      <div class="px-5 py-4">
        <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
          Erzeugung
        </div>
        <div class="mt-1 font-mono text-xl font-semibold">
          {formatKWh(totals.erz)}
        </div>
      </div>
    </Card>
    <Card>
      <div class="px-5 py-4">
        <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
          Eigenverbrauch
        </div>
        <div
          class="mt-1 font-mono text-xl font-semibold"
          style="color: var(--tr-green);"
        >
          {formatKWh(totals.ev)}
        </div>
      </div>
    </Card>
    <Card>
      <div class="px-5 py-4">
        <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
          Einspeisung
        </div>
        <div
          class="mt-1 font-mono text-xl font-semibold"
          style="color: var(--tr-sun);"
        >
          {formatKWh(totals.ei)}
        </div>
      </div>
    </Card>
    <Card>
      <div class="px-5 py-4">
        <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
          Netzbezug
        </div>
        <div
          class="mt-1 font-mono text-xl font-semibold"
          style="color: var(--tr-violet);"
        >
          {formatKWh(totals.nb)}
        </div>
      </div>
    </Card>
    <Card>
      <div class="px-5 py-4">
        <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
          Eigenverbrauchsquote
        </div>
        <div class="mt-1 font-mono text-xl font-semibold">
          {totals.erz > 0 ? formatPct(totals.ev / totals.erz) : "—"}
        </div>
      </div>
    </Card>
  </div>

  <Card>
    <CardHeader title="Aufschlüsselung" description={`${rows.length} Buckets, ${totals.tage} Tage`} />
    {#if rows.length === 0}
      <div class="px-5 py-6 text-sm text-[var(--tr-text-dim)]">
        Keine Daten im gewählten Zeitraum.
      </div>
    {:else}
      <table class="w-full text-sm">
        <thead
          class="bg-[var(--tr-surface2)] text-xs uppercase text-[var(--tr-text-dim)]"
        >
          <tr>
            <th class="px-5 py-2 text-left">Bucket</th>
            <th class="px-5 py-2 text-right">Erzeugung</th>
            <th class="px-5 py-2 text-right">Eigenverbr.</th>
            <th class="px-5 py-2 text-right">Einspeis.</th>
            <th class="px-5 py-2 text-right">Quote</th>
            <th class="w-[40%] px-5 py-2">Verteilung</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.bucket)}
            {@const quote = r.erzeugung_kwh > 0 ? r.eigenverbrauch_kwh / r.erzeugung_kwh : 0}
            {@const pct = maxErz > 0 ? (r.erzeugung_kwh / maxErz) * 100 : 0}
            <tr class="border-t border-[var(--tr-line)]">
              <td class="px-5 py-2 font-mono">{bucketLabel(r.bucket)}</td>
              <td class="px-5 py-2 text-right font-mono">{formatKWh(r.erzeugung_kwh)}</td>
              <td class="px-5 py-2 text-right font-mono">
                {formatKWh(r.eigenverbrauch_kwh)}
              </td>
              <td class="px-5 py-2 text-right font-mono">
                {formatKWh(r.einspeisung_kwh)}
              </td>
              <td class="px-5 py-2 text-right font-mono">{formatPct(quote)}</td>
              <td class="px-5 py-2">
                <div
                  class="relative h-3 w-full overflow-hidden rounded bg-[var(--tr-surface2)]"
                >
                  <div
                    class="absolute inset-y-0 left-0"
                    style:width={`${pct}%`}
                    style:background={`linear-gradient(90deg, var(--tr-green) 0%, var(--tr-green) ${
                      r.erzeugung_kwh > 0
                        ? (r.eigenverbrauch_kwh / r.erzeugung_kwh) * 100
                        : 0
                    }%, var(--tr-sun) ${
                      r.erzeugung_kwh > 0
                        ? (r.eigenverbrauch_kwh / r.erzeugung_kwh) * 100
                        : 0
                    }%, var(--tr-sun) 100%)`}
                  ></div>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </Card>
</div>
