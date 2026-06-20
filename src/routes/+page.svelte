<script lang="ts">
  import { onMount } from "svelte";
  import { aggregate, getDashboard, listDailyRange } from "$lib/api";
  import { formatEUR, formatKWh, formatPct, formatDateDE, todayISO } from "$lib/utils";
  import type { Aggregat, DailyProduction, DashboardSnapshot } from "$lib/types";
  import StatTile from "$lib/components/StatTile.svelte";
  import BarChart from "$lib/components/BarChart.svelte";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import {
    SunIcon,
    CalendarDaysIcon,
    CalendarRangeIcon,
    CalendarIcon,
    TrophyIcon,
    BanknoteIcon,
    PencilIcon,
    PiggyBankIcon,
  } from "@lucide/svelte";

  let snap = $state<DashboardSnapshot | null>(null);
  let last30 = $state<DailyProduction[]>([]);
  let monthly = $state<Aggregat[]>([]);
  let error = $state<string | null>(null);

  const currentYear = new Date().getFullYear();

  function pad2(n: number): string {
    return String(n).padStart(2, "0");
  }

  onMount(async () => {
    try {
      snap = await getDashboard();
      const today = new Date();
      const start = new Date(today);
      start.setDate(start.getDate() - 29);
      const startISO = `${start.getFullYear()}-${pad2(start.getMonth() + 1)}-${pad2(start.getDate())}`;
      [last30, monthly] = await Promise.all([
        listDailyRange(startISO, todayISO()),
        aggregate("monat", currentYear),
      ]);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });

  let evQuoteHeute = $derived(
    snap?.heute && snap.heute.erzeugung_kwh > 0
      ? snap.heute.eigenverbrauch_kwh / snap.heute.erzeugung_kwh
      : null,
  );

  /** Autarkiegrad = Eigenverbrauch / Gesamtverbrauch (EV + Netzbezug). */
  let autarkieHeute = $derived.by(() => {
    const h = snap?.heute;
    if (!h || h.netzbezug_kwh == null) return null;
    const gesamt = h.eigenverbrauch_kwh + h.netzbezug_kwh;
    return gesamt > 0 ? h.eigenverbrauch_kwh / gesamt : null;
  });

  let autarkieJahr = $derived.by(() => {
    const rows = last30;
    const withNetz = rows.filter((r) => r.netzbezug_kwh != null);
    if (withNetz.length === 0) return null;
    const ev = withNetz.reduce((s, r) => s + r.eigenverbrauch_kwh, 0);
    const nb = withNetz.reduce((s, r) => s + (r.netzbezug_kwh ?? 0), 0);
    const gesamt = ev + nb;
    return gesamt > 0 ? ev / gesamt : null;
  });

  let last30Series = $derived.by(() => {
    const map = new Map(last30.map((r) => [r.date, r]));
    const today = new Date();
    return Array.from({ length: 30 }, (_, idx) => {
      const offset = 29 - idx;
      const d = new Date(today);
      d.setDate(d.getDate() - offset);
      const iso = `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
      const existing = map.get(iso);
      return {
        label: pad2(d.getDate()),
        bottom: existing?.eigenverbrauch_kwh ?? 0,
        top: existing?.einspeisung_kwh ?? 0,
      };
    });
  });

  const MONTH_NAMES = [
    "Jan",
    "Feb",
    "Mär",
    "Apr",
    "Mai",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Okt",
    "Nov",
    "Dez",
  ];

  let monthlySeries = $derived.by(() => {
    const map = new Map(monthly.map((r) => [r.bucket.slice(5, 7), r]));
    return MONTH_NAMES.map((name, i) => {
      const existing = map.get(pad2(i + 1));
      return {
        label: name,
        bottom: existing?.eigenverbrauch_kwh ?? 0,
        top: existing?.einspeisung_kwh ?? 0,
      };
    });
  });

  let jahresSummen = $derived.by(() => {
    return monthly.reduce(
      (acc, r) => ({
        erz: acc.erz + r.erzeugung_kwh,
        ev: acc.ev + r.eigenverbrauch_kwh,
        ei: acc.ei + r.einspeisung_kwh,
        nb: acc.nb + r.netzbezug_kwh,
        spl: acc.spl + r.speicher_laden_kwh,
        spe: acc.spe + r.speicher_entladen_kwh,
        tage: acc.tage + r.tage,
      }),
      { erz: 0, ev: 0, ei: 0, nb: 0, spl: 0, spe: 0, tage: 0 },
    );
  });

  let evQuoteJahr = $derived(
    jahresSummen.erz > 0 ? jahresSummen.ev / jahresSummen.erz : null,
  );
  let autarkieJahresQuote = $derived.by(() => {
    const gesamt = jahresSummen.ev + jahresSummen.nb;
    return gesamt > 0 ? jahresSummen.ev / gesamt : null;
  });
  let durchschnittErzTag = $derived(
    jahresSummen.tage > 0 ? jahresSummen.erz / jahresSummen.tage : null,
  );
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Übersicht</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Stand: {formatDateDE(todayISO())}
      </p>
    </div>
    <a
      href="/erfassung"
      class="inline-flex h-9 items-center gap-1.5 rounded-md border border-transparent bg-[var(--tr-sun)] px-4 text-sm font-medium text-black hover:bg-[var(--tr-sun)]/90"
    >
      <PencilIcon class="size-4" />
      Tageswerte erfassen
    </a>
  </div>

  {#if error}
    <Card>
      <div class="p-5 text-sm text-[var(--tr-red)]">{error}</div>
    </Card>
  {:else if !snap}
    <div class="text-sm text-[var(--tr-text-dim)]">Lädt…</div>
  {:else}
    <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatTile
        label="Heute"
        value={formatKWh(snap.heute?.erzeugung_kwh ?? 0)}
        sub={snap.heute ? "Erzeugung" : "Noch keine Eingabe"}
        icon={SunIcon}
        accent="var(--tr-sun)"
      />
      <StatTile
        label="Letzte 7 Tage"
        value={formatKWh(snap.woche_kwh)}
        sub="Erzeugung gesamt"
        icon={CalendarDaysIcon}
        accent="var(--tr-green)"
      />
      <StatTile
        label="Diesen Monat"
        value={formatKWh(snap.monat_kwh)}
        sub="Erzeugung gesamt"
        icon={CalendarRangeIcon}
        accent="var(--tr-green)"
      />
      <StatTile
        label="Dieses Jahr"
        value={formatKWh(snap.jahr_kwh)}
        sub="Erzeugung gesamt"
        icon={CalendarIcon}
        accent="var(--tr-green)"
      />
    </div>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <Card class="lg:col-span-2">
        <CardHeader
          title="Heute"
          description={snap.heute
            ? `${formatDateDE(snap.heute.date)}`
            : "Noch keine Eingabe für heute"}
        />
        <div class="grid grid-cols-2 divide-[var(--tr-line)] md:grid-cols-4 md:divide-x">
          <div class="px-5 py-4">
            <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
              Erzeugung
            </div>
            <div class="mt-1 font-mono text-xl font-semibold">
              {formatKWh(snap.heute?.erzeugung_kwh ?? 0)}
            </div>
          </div>
          <div class="px-5 py-4">
            <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
              Eigenverbrauch
            </div>
            <div
              class="mt-1 font-mono text-xl font-semibold"
              style="color: var(--tr-green);"
            >
              {formatKWh(snap.heute?.eigenverbrauch_kwh ?? 0)}
            </div>
          </div>
          <div class="px-5 py-4">
            <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
              Einspeisung
            </div>
            <div
              class="mt-1 font-mono text-xl font-semibold"
              style="color: var(--tr-sun);"
            >
              {formatKWh(snap.heute?.einspeisung_kwh ?? 0)}
            </div>
          </div>
          <div class="px-5 py-4">
            <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
              Netzbezug
            </div>
            <div
              class="mt-1 font-mono text-xl font-semibold"
              style="color: var(--tr-violet);"
            >
              {snap.heute?.netzbezug_kwh != null
                ? formatKWh(snap.heute.netzbezug_kwh)
                : "—"}
            </div>
          </div>
        </div>
        {#if evQuoteHeute !== null || autarkieHeute !== null || autarkieJahr !== null}
          <div
            class="flex flex-wrap gap-x-6 gap-y-1 border-t border-[var(--tr-line)] px-5 py-3 text-xs text-[var(--tr-text-dim)]"
          >
            {#if evQuoteHeute !== null}
              <div>
                Eigenverbrauchsquote heute:
                <span class="font-mono font-medium text-[var(--tr-text)]">
                  {(evQuoteHeute * 100).toLocaleString("de-DE", {
                    maximumFractionDigits: 1,
                  })}%
                </span>
              </div>
            {/if}
            {#if autarkieHeute !== null}
              <div>
                Autarkiegrad heute:
                <span class="font-mono font-medium text-[var(--tr-text)]">
                  {(autarkieHeute * 100).toLocaleString("de-DE", {
                    maximumFractionDigits: 1,
                  })}%
                </span>
              </div>
            {/if}
            {#if autarkieJahr !== null}
              <div>
                Autarkiegrad 30 Tage:
                <span class="font-mono font-medium text-[var(--tr-text)]">
                  {(autarkieJahr * 100).toLocaleString("de-DE", {
                    maximumFractionDigits: 1,
                  })}%
                </span>
              </div>
            {/if}
          </div>
        {/if}
      </Card>

      <div class="space-y-4">
        <StatTile
          label="Bester Tag (Max)"
          value={snap.max_tag ? formatKWh(snap.max_tag.erzeugung_kwh) : "—"}
          sub={snap.max_tag ? formatDateDE(snap.max_tag.date) : "Noch keine Daten"}
          icon={TrophyIcon}
          accent="var(--tr-warning)"
        />
        {#if snap.betreiber_modus === "privat"}
          <StatTile
            label="Ersparnis Jahr"
            value={formatEUR(snap.einsparung_jahr)}
            sub="Eigenverbrauch × Strom-Bezugspreis"
            icon={PiggyBankIcon}
            accent="var(--tr-green)"
          />
        {:else}
          <StatTile
            label="Einnahmen Jahr (netto)"
            value={formatEUR(snap.einnahmen_jahr)}
            sub="Bayernwerk-Auszahlungen"
            icon={BanknoteIcon}
            accent="var(--tr-green)"
          />
          {#if snap.einsparung_jahr > 0}
            <StatTile
              label="Ersparnis Jahr"
              value={formatEUR(snap.einsparung_jahr)}
              sub="vermiedener Netzbezug (informativ)"
              icon={PiggyBankIcon}
              accent="var(--tr-green-dim)"
            />
          {/if}
        {/if}
      </div>
    </div>

    <Card>
      <CardHeader
        title="Jahr {currentYear}"
        description={`Summen über ${jahresSummen.tage} erfasste Tage`}
      />
      <div class="grid grid-cols-2 divide-[var(--tr-line)] md:grid-cols-4 lg:grid-cols-7 md:divide-x">
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            Erzeugung
          </div>
          <div class="mt-1 font-mono text-lg font-semibold">
            {formatKWh(jahresSummen.erz)}
          </div>
        </div>
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            Eigenverbrauch
          </div>
          <div class="mt-1 font-mono text-lg font-semibold" style="color: var(--tr-green);">
            {formatKWh(jahresSummen.ev)}
          </div>
        </div>
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            Einspeisung
          </div>
          <div class="mt-1 font-mono text-lg font-semibold" style="color: var(--tr-sun);">
            {formatKWh(jahresSummen.ei)}
          </div>
        </div>
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            Netzbezug
          </div>
          <div class="mt-1 font-mono text-lg font-semibold" style="color: var(--tr-violet);">
            {jahresSummen.nb > 0 ? formatKWh(jahresSummen.nb) : "—"}
          </div>
        </div>
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            Speicher ↓
          </div>
          <div class="mt-1 font-mono text-lg font-semibold text-[var(--tr-text-dim)]">
            {jahresSummen.spl > 0 ? formatKWh(jahresSummen.spl) : "—"}
          </div>
        </div>
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            Speicher ↑
          </div>
          <div class="mt-1 font-mono text-lg font-semibold text-[var(--tr-text-dim)]">
            {jahresSummen.spe > 0 ? formatKWh(jahresSummen.spe) : "—"}
          </div>
        </div>
        <div class="px-5 py-4">
          <div class="text-xs uppercase tracking-wide text-[var(--tr-text-dim)]">
            ⌀ pro Tag
          </div>
          <div class="mt-1 font-mono text-lg font-semibold">
            {durchschnittErzTag !== null ? formatKWh(durchschnittErzTag) : "—"}
          </div>
        </div>
      </div>
      {#if evQuoteJahr !== null || autarkieJahresQuote !== null}
        <div
          class="flex flex-wrap gap-x-6 gap-y-1 border-t border-[var(--tr-line)] px-5 py-3 text-xs text-[var(--tr-text-dim)]"
        >
          {#if evQuoteJahr !== null}
            <div>
              Eigenverbrauchsquote:
              <span class="font-mono font-medium text-[var(--tr-text)]">
                {formatPct(evQuoteJahr)}
              </span>
            </div>
          {/if}
          {#if autarkieJahresQuote !== null}
            <div>
              Autarkiegrad:
              <span class="font-mono font-medium text-[var(--tr-text)]">
                {formatPct(autarkieJahresQuote)}
              </span>
            </div>
          {/if}
        </div>
      {/if}
    </Card>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <Card>
        <CardHeader
          title="Letzte 30 Tage"
          description="Tageserzeugung — Eigenverbrauch und Einspeisung gestapelt"
        />
        <BarChart data={last30Series} showEveryNthLabel={5} />
      </Card>

      <Card>
        <CardHeader
          title="Monate {currentYear}"
          description="Monatssumme — Eigenverbrauch und Einspeisung gestapelt"
        />
        <BarChart data={monthlySeries} />
      </Card>
    </div>
  {/if}
</div>
