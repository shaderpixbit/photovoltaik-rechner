<script lang="ts">
  import { onMount } from "svelte";
  import { getUstva } from "$lib/api";
  import type { UstvaReport } from "$lib/types";
  import { formatEUR } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { PrinterIcon } from "@lucide/svelte";
  import { formatDateDE, todayISO } from "$lib/utils";

  const currentYear = new Date().getFullYear();
  let jahrSel = $state(String(currentYear));
  let jahr = $derived(Number(jahrSel));
  let monat = $state<number | null>(null);
  let report = $state<UstvaReport | null>(null);
  let error = $state<string | null>(null);

  const jahre = Array.from({ length: 10 }, (_, i) => currentYear - i);
  const MONATE = [
    { value: "alle", label: "Ganzes Jahr" },
    { value: "1", label: "Januar" },
    { value: "2", label: "Februar" },
    { value: "3", label: "März" },
    { value: "4", label: "April" },
    { value: "5", label: "Mai" },
    { value: "6", label: "Juni" },
    { value: "7", label: "Juli" },
    { value: "8", label: "August" },
    { value: "9", label: "September" },
    { value: "10", label: "Oktober" },
    { value: "11", label: "November" },
    { value: "12", label: "Dezember" },
  ];
  let monatSel = $state("alle");

  async function load() {
    error = null;
    try {
      monat = monatSel === "alle" ? null : Number(monatSel);
      report = await getUstva(jahr, monat);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
  onMount(load);
  $effect(() => {
    jahrSel;
    monatSel;
    load();
  });

  const MODUS_LABEL: Record<string, string> = {
    regel: "Regelbesteuerung 19%",
    kleinunternehmer: "Kleinunternehmer §19 UStG",
    nullsteuer: "Nullsteuersatz §12(3) UStG",
  };

  let zeitraumLabel = $derived(
    monatSel === "alle"
      ? `Jahr ${jahr}`
      : `${MONATE.find((m) => m.value === monatSel)?.label ?? monatSel} ${jahr}`,
  );

  function druck() {
    window.print();
  }
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between gap-4" data-print="hide">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">
        Umsatzsteuer-Voranmeldung
      </h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Berechnung für Finanzamt. Modus wird je Periode aus den Einstellungen
        gewählt.
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
      <div class="w-44 space-y-1.5">
        <Label>Zeitraum</Label>
        <Select bind:value={monatSel} options={MONATE} />
      </div>
      <Button variant="ghost" onclick={druck}>
        <PrinterIcon class="size-4" />Drucken / PDF
      </Button>
    </div>
  </div>

  <div data-print="show" class="border-b border-[var(--tr-line)] pb-3">
    <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
      Umsatzsteuer-Voranmeldung
    </div>
    <div class="text-lg font-semibold">{zeitraumLabel}</div>
    <div class="text-xs text-[var(--tr-text-dim)]">
      Stand: {formatDateDE(todayISO())}
    </div>
  </div>

  {#if error}
    <Card><div class="p-5 text-sm text-[var(--tr-red)]">{error}</div></Card>
  {:else if !report}
    <div class="text-sm text-[var(--tr-text-dim)]">Lädt…</div>
  {:else}
    <Card>
      <CardHeader
        title={`Modus: ${MODUS_LABEL[report.modus] ?? report.modus}`}
        description="Maßgeblich ist der USt-Modus am Periodenende."
      />
      <dl class="divide-y divide-[var(--tr-line)]">
        <div class="flex items-center justify-between px-5 py-3">
          <dt class="text-sm">
            USt aus Einspeisevergütung
            <span class="ml-1 text-xs text-[var(--tr-text-faint)]">
              (Bayernwerk-Auszahlungen)
            </span>
          </dt>
          <dd class="font-mono text-sm">{formatEUR(report.ust_einnahmen)}</dd>
        </div>
        <div class="flex items-center justify-between px-5 py-3">
          <dt class="text-sm">
            USt aus Eigenverbrauch
            <span class="ml-1 text-xs text-[var(--tr-text-faint)]">
              (unentgeltliche Wertabgabe)
            </span>
          </dt>
          <dd class="font-mono text-sm">{formatEUR(report.ust_eigenverbrauch)}</dd>
        </div>
        <div class="flex items-center justify-between px-5 py-3">
          <dt class="text-sm">Abzügl. Vorsteuer</dt>
          <dd class="font-mono text-sm">−{formatEUR(report.vorsteuer)}</dd>
        </div>
        <div
          class="flex items-center justify-between px-5 py-4 text-base font-semibold"
          style:background={report.zahllast >= 0
            ? "var(--tr-warning-bg)"
            : "var(--tr-green-bg)"}
        >
          <dt>{report.zahllast >= 0 ? "Zahllast an Finanzamt" : "Erstattung"}</dt>
          <dd
            class="font-mono"
            style:color={report.zahllast >= 0
              ? "var(--tr-warning)"
              : "var(--tr-green-dim)"}
          >
            {formatEUR(Math.abs(report.zahllast))}
          </dd>
        </div>
      </dl>
    </Card>

    {#if report.modus === "kleinunternehmer"}
      <Card>
        <div class="px-5 py-4 text-sm text-[var(--tr-text-dim)]">
          <strong>Kleinunternehmerregelung §19 UStG</strong> aktiv — keine USt-Erhebung,
          keine UStVA fällig. Auf Rechnungen muss der Hinweis stehen, dass keine
          USt ausgewiesen wird.
        </div>
      </Card>
    {:else if report.modus === "nullsteuer"}
      <Card>
        <div class="px-5 py-4 text-sm text-[var(--tr-text-dim)]">
          <strong>Nullsteuersatz §12 Abs. 3 UStG</strong> für Anschaffung der PV-Anlage:
          0% USt seit 01.01.2023. Die Einspeisung bleibt regelbesteuert; die
          Eigenverbrauchsbesteuerung (unentgeltliche Wertabgabe) entfällt seit
          2023 (BMF-Schreiben 27.02.2023).
        </div>
      </Card>
    {/if}
  {/if}
</div>
