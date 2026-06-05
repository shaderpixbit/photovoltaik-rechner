<script lang="ts">
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { exportBuchungenCsv, getEuer } from "$lib/api";
  import type { EuerReport } from "$lib/types";
  import { formatDateDE, formatEUR, todayISO } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { DownloadIcon, PrinterIcon } from "@lucide/svelte";

  const currentYear = new Date().getFullYear();
  let jahrSel = $state(String(currentYear));
  let jahr = $derived(Number(jahrSel));
  let report = $state<EuerReport | null>(null);
  let error = $state<string | null>(null);

  const jahre = Array.from({ length: 10 }, (_, i) => currentYear - i);

  async function load() {
    error = null;
    try {
      report = await getEuer(jahr);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
  onMount(load);
  $effect(() => {
    jahrSel;
    load();
  });

  let einnahmen = $derived(
    report
      ? report.einnahmen_einspeisung_netto + report.einnahmen_eigenverbrauch_netto
      : 0,
  );
  let ausgaben = $derived(
    report ? report.ausgaben_betrieb_netto + report.ausgaben_afa : 0,
  );

  let busy = $state(false);
  let exportMsg = $state<string | null>(null);

  async function exportCsv() {
    busy = true;
    exportMsg = null;
    try {
      const path = await save({
        defaultPath: `buchungen-${jahr}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return;
      const n = await exportBuchungenCsv(path, jahr);
      exportMsg = `${n} Buchung(en) nach CSV exportiert.`;
      setTimeout(() => (exportMsg = null), 4000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function druck() {
    window.print();
  }
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between" data-print="hide">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Einnahmen-Überschuss-Rechnung</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Netto-Werte (Anlage EÜR). Eigenverbrauch nur als unentgeltliche Wertabgabe relevant.
      </p>
    </div>
    <div class="flex items-end gap-3">
      <div class="w-32 space-y-1.5">
        <Label>Jahr</Label>
        <Select
          bind:value={jahrSel}
          options={jahre.map((y) => ({ value: String(y), label: String(y) }))}
        />
      </div>
      <Button variant="ghost" onclick={exportCsv} disabled={busy}>
        <DownloadIcon class="size-4" />CSV
      </Button>
      <Button variant="ghost" onclick={druck}>
        <PrinterIcon class="size-4" />Drucken / PDF
      </Button>
    </div>
  </div>

  {#if exportMsg}
    <div
      class="rounded-md border border-[var(--tr-green)] bg-[var(--tr-green-bg)] px-4 py-2 text-sm"
      style="color: var(--tr-green-dim);"
      data-print="hide"
    >
      {exportMsg}
    </div>
  {/if}

  <div data-print="show" class="border-b border-[var(--tr-line)] pb-3">
    <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
      Einnahmen-Überschuss-Rechnung
    </div>
    <div class="text-lg font-semibold">Jahr {jahr}</div>
    <div class="text-xs text-[var(--tr-text-dim)]">
      Stand: {formatDateDE(todayISO())}
    </div>
  </div>

  {#if error}
    <Card><div class="p-5 text-sm text-[var(--tr-red)]">{error}</div></Card>
  {:else if !report}
    <div class="text-sm text-[var(--tr-text-dim)]">Lädt…</div>
  {:else}
    {#if !report.est_pflichtig}
      <Card>
        <div
          class="flex items-start gap-3 p-5"
          style="background: var(--tr-yellow-bg, #fef3c7); color: var(--tr-text);"
        >
          <div class="text-2xl leading-none">i</div>
          <div class="space-y-1 text-sm">
            <div class="font-semibold">Einkommensteuer-befreit ({jahr})</div>
            <div class="text-[var(--tr-text-dim)]">
              {report.est_befreiungsgrund ??
                "Betreiber-Status „privat“ am Jahresende — keine EÜR-Pflicht."}
            </div>
            <div class="text-xs text-[var(--tr-text-dim)]">
              Die Werte unten sind nur informativ und fließen nicht in die
              Einkommensteuer-Erklärung. Die UStVA bleibt davon unberührt.
            </div>
          </div>
        </div>
      </Card>
    {/if}

    <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
      <Card>
        <CardHeader title="Einnahmen" />
        <dl class="divide-y divide-[var(--tr-line)]">
          <div class="flex items-center justify-between px-5 py-3">
            <dt class="text-sm">Einspeisevergütung (netto)</dt>
            <dd class="font-mono text-sm">
              {formatEUR(report.einnahmen_einspeisung_netto)}
            </dd>
          </div>
          <div class="flex items-center justify-between px-5 py-3">
            <dt class="text-sm">Unentgeltl. Wertabgabe (Eigenverbrauch)</dt>
            <dd class="font-mono text-sm">
              {formatEUR(report.einnahmen_eigenverbrauch_netto)}
            </dd>
          </div>
          <div
            class="flex items-center justify-between px-5 py-3 font-semibold"
            style="background: var(--tr-green-bg);"
          >
            <dt>Summe Einnahmen netto</dt>
            <dd class="font-mono">{formatEUR(einnahmen)}</dd>
          </div>
        </dl>
      </Card>

      <Card>
        <CardHeader title="Ausgaben" />
        <dl class="divide-y divide-[var(--tr-line)]">
          <div class="flex items-center justify-between px-5 py-3">
            <dt class="text-sm">Betriebsausgaben (netto)</dt>
            <dd class="font-mono text-sm">
              {formatEUR(report.ausgaben_betrieb_netto)}
            </dd>
          </div>
          <div class="flex items-center justify-between px-5 py-3">
            <dt class="text-sm">Abschreibung (AfA)</dt>
            <dd class="font-mono text-sm">{formatEUR(report.ausgaben_afa)}</dd>
          </div>
          <div
            class="flex items-center justify-between px-5 py-3 font-semibold"
            style="background: var(--tr-red-bg);"
          >
            <dt>Summe Ausgaben</dt>
            <dd class="font-mono">{formatEUR(ausgaben)}</dd>
          </div>
        </dl>
      </Card>
    </div>

    <Card>
      <CardHeader
        title={`Gewinn vor Steuern ${jahr}`}
        description="Bemessungsgrundlage für die Einkommensteuer (Anlage G / EÜR)."
      />
      <div
        class="px-5 py-6 text-center font-mono text-3xl font-semibold"
        style:color={report.gewinn_vor_steuern >= 0
          ? "var(--tr-green-dim)"
          : "var(--tr-red)"}
      >
        {formatEUR(report.gewinn_vor_steuern)}
      </div>
    </Card>

    <Card>
      <CardHeader
        title="USt-Kontrolle"
        description="Vereinnahmte USt und gezahlte Vorsteuer im Jahr — Detail siehe Umsatzsteuer-Modul."
      />
      <dl class="grid grid-cols-2 divide-x divide-[var(--tr-line)]">
        <div class="px-5 py-4">
          <dt class="text-xs uppercase text-[var(--tr-text-dim)]">USt vereinnahmt</dt>
          <dd class="mt-1 font-mono text-lg">{formatEUR(report.einnahmen_ust)}</dd>
        </div>
        <div class="px-5 py-4">
          <dt class="text-xs uppercase text-[var(--tr-text-dim)]">Vorsteuer</dt>
          <dd class="mt-1 font-mono text-lg">{formatEUR(report.vorsteuer)}</dd>
        </div>
      </dl>
    </Card>
  {/if}
</div>
