<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    deleteDaily,
    getDaily,
    getSettings,
    importFromVendor,
    listDailyRange,
    upsertDaily,
  } from "$lib/api";
  import type { DailyProduction, VendorKind } from "$lib/types";
  import { formatDateDE, formatKWh, todayISO } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import DateField from "$lib/components/ui/DateField.svelte";
  import MonthField from "$lib/components/ui/MonthField.svelte";
  import { CloudDownloadIcon, SaveIcon, Trash2Icon } from "@lucide/svelte";

  type Mode = "tag" | "monat" | "jahr";

  const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
  const ISO_MONTH = /^\d{4}-\d{2}$/;

  let mode = $state<Mode>("tag");
  let dateValue = $state(todayISO());
  let monthValue = $state(todayISO().slice(0, 7));
  let yearValue = $state<number | "">(Number(todayISO().slice(0, 4)));

  let erzeugung = $state<number | "">("");
  let einspeisung = $state<number | "">("");
  let netzbezug = $state<number | "">("");
  let speicherLaden = $state<number | "">("");
  let speicherEntladen = $state<number | "">("");
  let notiz = $state("");

  let eigenverbrauchComputed = $derived.by(() => {
    const e = Number(erzeugung) || 0;
    const ei = Number(einspeisung) || 0;
    return Math.max(0, round1(e - ei));
  });
  let eigenverbrauchNegativ = $derived(
    (Number(erzeugung) || 0) < (Number(einspeisung) || 0),
  );

  let recent = $state<DailyProduction[]>([]);
  let existingInPeriod = $state(0);
  let busy = $state(false);
  let toast = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let vendor = $state<VendorKind>("none");

  const VENDOR_LABELS: Record<VendorKind, string> = {
    none: "kein API",
    anker: "Anker",
    solaredge: "SolarEdge",
  };

  // Sidecar-Import-Status: Startzeit (epoch ms) wenn ein Import laeuft, sonst
  // null. `importElapsed` wird per Effect alle 500ms aktualisiert, damit der
  // Timer in der UI zaehlt — ohne dass wir auf jeden Tick Re-Renders der
  // Eingabe-Felder ausloesen.
  let importStartedAt = $state<number | null>(null);
  let importElapsedSec = $state(0);
  let importEstSec = $state(0);

  // Live-Progress aus dem Python-Sidecar (via stderr -> Rust -> Tauri-Event).
  let importProgressMsg = $state("");
  let importProgressDone = $state(0);
  let importProgressTotal = $state(0);
  let unlistenProgress: UnlistenFn | null = null;

  $effect(() => {
    if (importStartedAt === null) {
      importElapsedSec = 0;
      return;
    }
    const started = importStartedAt;
    const handle = setInterval(() => {
      importElapsedSec = Math.floor((Date.now() - started) / 1000);
    }, 500);
    return () => clearInterval(handle);
  });

  function formatDuration(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    return `${m}:${String(s).padStart(2, "0")}`;
  }

  function showToast(kind: "ok" | "err", text: string) {
    toast = { kind, text };
    setTimeout(() => (toast = null), 3500);
  }

  function pad2(n: number): string {
    return String(n).padStart(2, "0");
  }

  function daysInMonth(year: number, month: number): number {
    return new Date(year, month, 0).getDate();
  }

  function periodRange(): { from: string; to: string } | null {
    if (mode === "tag") {
      if (!ISO_DATE.test(dateValue)) return null;
      return { from: dateValue, to: dateValue };
    }
    if (mode === "monat") {
      if (!ISO_MONTH.test(monthValue)) return null;
      const [y, m] = monthValue.split("-").map(Number);
      const dim = daysInMonth(y, m);
      return { from: `${monthValue}-01`, to: `${monthValue}-${pad2(dim)}` };
    }
    const y = Number(yearValue);
    if (!Number.isFinite(y) || y < 2000 || y > 2100) return null;
    return { from: `${y}-01-01`, to: `${y}-12-31` };
  }

  function periodDays(): string[] {
    const r = periodRange();
    if (!r) return [];
    if (mode === "tag") return [r.from];
    if (mode === "monat") {
      const [y, m] = monthValue.split("-").map(Number);
      const dim = daysInMonth(y, m);
      return Array.from(
        { length: dim },
        (_, i) => `${monthValue}-${pad2(i + 1)}`,
      );
    }
    const y = Number(yearValue);
    const days: string[] = [];
    for (let m = 1; m <= 12; m++) {
      const dim = daysInMonth(y, m);
      for (let d = 1; d <= dim; d++) {
        days.push(`${y}-${pad2(m)}-${pad2(d)}`);
      }
    }
    return days;
  }

  let periodLabel = $derived.by(() => {
    if (mode === "tag") return formatDateDE(dateValue);
    if (mode === "monat") {
      if (!ISO_MONTH.test(monthValue)) return monthValue;
      const [y, m] = monthValue.split("-").map(Number);
      const name = new Intl.DateTimeFormat("de-DE", {
        month: "long",
        year: "numeric",
      }).format(new Date(y, m - 1, 1));
      return name;
    }
    return String(yearValue);
  });

  let periodDayCount = $derived(periodDays().length);

  async function loadRecent() {
    const today = new Date();
    const from = new Date(today);
    from.setDate(from.getDate() - 29);
    const fromISO = `${from.getFullYear()}-${pad2(from.getMonth() + 1)}-${pad2(from.getDate())}`;
    try {
      recent = await listDailyRange(fromISO, todayISO());
      recent.sort((a, b) => b.date.localeCompare(a.date));
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
    }
  }

  async function loadForPeriod() {
    const r = periodRange();
    if (!r) {
      erzeugung = "";
      einspeisung = "";
      notiz = "";
      existingInPeriod = 0;
      return;
    }
    try {
      if (mode === "tag") {
        const existing = await getDaily(r.from);
        if (existing) {
          erzeugung = existing.erzeugung_kwh;
          einspeisung = existing.einspeisung_kwh;
          netzbezug = existing.netzbezug_kwh ?? "";
          speicherLaden = existing.speicher_laden_kwh ?? "";
          speicherEntladen = existing.speicher_entladen_kwh ?? "";
          notiz = existing.notiz ?? "";
          existingInPeriod = 1;
        } else {
          erzeugung = "";
          einspeisung = "";
          netzbezug = "";
          speicherLaden = "";
          speicherEntladen = "";
          notiz = "";
          existingInPeriod = 0;
        }
      } else {
        const rows = await listDailyRange(r.from, r.to);
        existingInPeriod = rows.length;
        if (rows.length === 0) {
          erzeugung = "";
          einspeisung = "";
          netzbezug = "";
          speicherLaden = "";
          speicherEntladen = "";
          notiz = "";
        } else {
          erzeugung = round1(rows.reduce((s, x) => s + x.erzeugung_kwh, 0));
          einspeisung = round1(
            rows.reduce((s, x) => s + x.einspeisung_kwh, 0),
          );
          const netzSum = rows.reduce((s, x) => s + (x.netzbezug_kwh ?? 0), 0);
          netzbezug = netzSum > 0 ? round1(netzSum) : "";
          const spLadSum = rows.reduce((s, x) => s + (x.speicher_laden_kwh ?? 0), 0);
          speicherLaden = spLadSum > 0 ? round1(spLadSum) : "";
          const spEntSum = rows.reduce((s, x) => s + (x.speicher_entladen_kwh ?? 0), 0);
          speicherEntladen = spEntSum > 0 ? round1(spEntSum) : "";
          notiz = "";
        }
      }
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
    }
  }

  function round1(v: number): number {
    return Math.round(v * 10) / 10;
  }

  onMount(async () => {
    await loadRecent();
    try {
      const s = await getSettings();
      vendor = (s.vendor as VendorKind) ?? "none";
    } catch {
      vendor = "none";
    }
  });

  $effect(() => {
    // Re-load when the chosen period changes. Read all relevant state up-front
    // so the effect tracks every input that defines the period.
    void mode;
    void dateValue;
    void monthValue;
    void yearValue;
    loadForPeriod();
  });

  async function save() {
    const days = periodDays();
    if (days.length === 0) {
      showToast("err", "Ungültiger Zeitraum.");
      return;
    }
    if (eigenverbrauchNegativ) {
      showToast("err", "Einspeisung darf nicht größer als Erzeugung sein.");
      return;
    }
    if (
      (mode === "monat" || mode === "jahr") &&
      existingInPeriod > 0 &&
      !confirm(
        `Im Zeitraum ${periodLabel} existieren bereits ${existingInPeriod} Tageseinträge. ` +
          `Diese werden durch die gleichmäßige Verteilung überschrieben. Fortfahren?`,
      )
    ) {
      return;
    }

    const totalErz = Number(erzeugung) || 0;
    const totalEv = eigenverbrauchComputed;
    const totalEi = Number(einspeisung) || 0;
    const hasNetz = netzbezug !== "" && Number.isFinite(Number(netzbezug));
    const totalNetz = hasNetz ? Number(netzbezug) : 0;
    const hasSpLad = speicherLaden !== "" && Number.isFinite(Number(speicherLaden));
    const totalSpLad = hasSpLad ? Number(speicherLaden) : 0;
    const hasSpEnt = speicherEntladen !== "" && Number.isFinite(Number(speicherEntladen));
    const totalSpEnt = hasSpEnt ? Number(speicherEntladen) : 0;
    const n = days.length;
    const perErz = totalErz / n;
    const perEv = totalEv / n;
    const perEi = totalEi / n;
    const perNetz = totalNetz / n;
    const perSpLad = totalSpLad / n;
    const perSpEnt = totalSpEnt / n;
    const note = notiz.trim() || null;

    busy = true;
    try {
      for (const d of days) {
        const entry: DailyProduction = {
          date: d,
          erzeugung_kwh: mode === "tag" ? totalErz : perErz,
          eigenverbrauch_kwh: mode === "tag" ? totalEv : perEv,
          einspeisung_kwh: mode === "tag" ? totalEi : perEi,
          netzbezug_kwh: hasNetz ? (mode === "tag" ? totalNetz : perNetz) : null,
          speicher_laden_kwh: hasSpLad ? (mode === "tag" ? totalSpLad : perSpLad) : null,
          speicher_entladen_kwh: hasSpEnt ? (mode === "tag" ? totalSpEnt : perSpEnt) : null,
          notiz: note,
        };
        await upsertDaily(entry);
      }
      const suffix =
        mode === "tag"
          ? formatDateDE(days[0])
          : `${periodLabel} (${n} Tage à ${formatKWh(perErz)})`;
      showToast("ok", `Gespeichert: ${suffix}`);
      await loadRecent();
      await loadForPeriod();
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
    } finally {
      busy = false;
    }
  }

  async function remove(d: string) {
    if (!confirm(`Eintrag ${formatDateDE(d)} wirklich löschen?`)) return;
    try {
      await deleteDaily(d);
      await loadRecent();
      await loadForPeriod();
      showToast("ok", "Gelöscht.");
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
    }
  }

  async function tryImport() {
    const r = periodRange();
    if (!r) {
      showToast("err", "Ungueltiger Zeitraum.");
      return;
    }
    // Per-Tag-Strategie: 2 API-Calls + 0.4s sleep pro Tag + ~0.2s HTTP
    // = ca. 1s/Tag. Plus ~3s Login-Overhead.
    const days = periodDays().length;
    const estSec = days + 3;
    if (
      days > 31 &&
      !confirm(
        `Import von ${days} Tagen (${r.from} bis ${r.to}) — ca. ` +
          `${formatDuration(estSec)} min Laufzeit (2 Calls/Tag wegen ` +
          `Anker-Rate-Limit). Fortfahren?`,
      )
    ) {
      return;
    }
    busy = true;
    importStartedAt = Date.now();
    importEstSec = estSec;
    importProgressMsg = "Starte Sidecar…";
    importProgressDone = 0;
    importProgressTotal = 0;
    // Garantieren dass der Banner gerendert ist bevor wir blockierende
    // Awaits machen — Svelte 5 batched State-Mutations, await listen()
    // koennte sonst die DOM-Aktualisierung verzoegern.
    await tick();
    // Live-Updates vom Sidecar abonnieren BEVOR die invoke laeuft — sonst
    // koennten frueh emittierte Events verloren gehen.
    unlistenProgress = await listen<{
      progress: string;
      done: number;
      total: number;
    }>("anker-import-progress", (e) => {
      // console.log laesst sich in DevTools (F12 in dev) live mitlesen —
      // wenn hier nichts erscheint, kommt das Event nicht vom Rust an.
      console.debug("[anker-import-progress]", e.payload);
      importProgressMsg = e.payload.progress;
      importProgressDone = e.payload.done;
      importProgressTotal = e.payload.total;
    });
    try {
      const res = await importFromVendor(r.from, r.to);
      let msg = `${res.imported} Tage importiert in ${formatDuration(importElapsedSec)}`;
      if (res.skipped > 0) msg += `, ${res.skipped} uebersprungen`;
      if (res.warnings.length > 0) msg += ` (${res.warnings.length} Hinweise)`;
      showToast(res.errors.length > 0 ? "err" : "ok", msg);
      await loadRecent();
      await loadForPeriod();
    } catch (e) {
      showToast("err", e instanceof Error ? e.message : String(e));
      importProgressMsg = "Abgebrochen: " + (e instanceof Error ? e.message : String(e));
    } finally {
      busy = false;
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
      // Card noch 2s sichtbar lassen, damit der User auch bei schnellem /
      // fehlgeschlagenem Import den Endzustand wahrnimmt — sonst flasht der
      // Banner zu kurz auf.
      setTimeout(() => {
        importStartedAt = null;
      }, 2000);
    }
  }

  function selectRow(d: string) {
    mode = "tag";
    dateValue = d;
  }
</script>

<div class="space-y-6">
  <div class="flex items-end justify-between">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Erfassung</h1>
      <p class="text-sm text-[var(--tr-text-dim)]">
        Manuelle Eingabe als Tag, Monat oder Jahr — oder Import aus Hersteller-API.
      </p>
    </div>
    <Button variant="ghost" onclick={tryImport}
      disabled={busy || vendor === "none"}
      title={vendor === "none"
        ? "Kein Hersteller-API ausgewaehlt. Aktiviere Anker oder SolarEdge unter Einstellungen → Hersteller-API."
        : "Importiert Tageswerte fuer den unten gewaehlten Zeitraum. Heute/gestern liefert die API oft noch keine finale Tagessumme — diese Tage ggf. spaeter nachholen."}>
      <CloudDownloadIcon class="size-4" />
      API-Import ({VENDOR_LABELS[vendor]} · {periodLabel})
    </Button>
  </div>

  {#if importStartedAt !== null}
    {@const realPct = importProgressTotal > 0
      ? Math.round((importProgressDone / importProgressTotal) * 100)
      : 0}
    {@const estPct = importEstSec > 0
      ? Math.min(99, Math.round((importElapsedSec / importEstSec) * 100))
      : 0}
    {@const pct = importProgressTotal > 0 ? realPct : estPct}
    <!-- Sticky an die Top-Zone, damit der Banner auch bei langer Seite
         immer sichtbar bleibt waehrend der Sidecar laeuft. -->
    <div class="sticky top-2 z-30">
      <div
        class="rounded-lg border-2 shadow-lg"
        style="background: var(--tr-surface); border-color: var(--tr-sun);"
      >
        <div class="px-5 py-4">
          <div class="flex items-center justify-between text-sm">
            <span class="inline-flex items-center gap-2 font-medium">
              <span class="inline-block size-2.5 animate-pulse rounded-full"
                style="background: var(--tr-sun);"></span>
              {importProgressMsg || "Anker-Cloud-Import laeuft…"}
            </span>
            <span class="font-mono text-xs text-[var(--tr-text-dim)]">
              {#if importProgressTotal > 0}
                {importProgressDone}/{importProgressTotal} Calls ·
              {/if}
              {formatDuration(importElapsedSec)} / ~{formatDuration(importEstSec)}
            </span>
          </div>
          <div class="mt-2 h-2 w-full overflow-hidden rounded-full"
            style="background: var(--tr-surface2);">
            <div
              class="h-full transition-all duration-300"
              style="width: {pct}%; background: var(--tr-sun);"
            ></div>
          </div>
          <p class="mt-2 text-xs text-[var(--tr-text-faint)]">
            Sidecar holt Tageswerte tagweise (2 Calls/Tag, 0.4s Pause wegen
            Rate-Limit). Tage ohne Solar-Daten werden uebersprungen.
            {#if importProgressTotal === 0 && importElapsedSec > 5}
              <br /><strong>Hinweis:</strong> nach 5s noch keine Live-Events vom
              Sidecar — eventuell altes Rust-Binary. <code>bun run tauri dev</code>
              neu starten falls Probleme.
            {/if}
          </p>
        </div>
      </div>
    </div>
  {/if}

  <Card>
    <CardHeader
      title="Zeitraum & Werte"
      description="Bei Monat/Jahr werden die Summen gleichmäßig auf alle Tage verteilt."
    />

    <div class="flex flex-wrap items-center gap-2 px-5 pt-5">
      {#each [{ k: "tag", label: "Tag" }, { k: "monat", label: "Monat" }, { k: "jahr", label: "Jahr" }] as opt (opt.k)}
        <button
          type="button"
          class="h-8 rounded-md border px-3 text-sm transition-colors"
          class:border-transparent={mode !== opt.k}
          style:background={mode === opt.k
            ? "var(--tr-sun)"
            : "var(--tr-surface)"}
          style:color={mode === opt.k ? "black" : "var(--tr-text-dim)"}
          style:border-color={mode === opt.k
            ? "transparent"
            : "var(--tr-line)"}
          onclick={() => (mode = opt.k as Mode)}
        >
          {opt.label}
        </button>
      {/each}
      <span class="ml-2 text-xs text-[var(--tr-text-dim)]">
        {periodLabel} · {periodDayCount}
        {periodDayCount === 1 ? "Tag" : "Tage"}
      </span>
    </div>

    <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-5">
      <div class="space-y-1.5">
        {#if mode === "tag"}
          <Label for="date">Datum</Label>
          <DateField id="date" bind:value={dateValue} />
        {:else if mode === "monat"}
          <Label for="month">Monat</Label>
          <MonthField id="month" bind:value={monthValue} />
        {:else}
          <Label for="year">Jahr</Label>
          <Input
            id="year"
            type="number"
            min="2000"
            max="2100"
            step="1"
            bind:value={yearValue}
          />
        {/if}
      </div>
      <div class="space-y-1.5">
        <Label for="erz">
          Erzeugung (kWh)
          {#if mode !== "tag"}<span class="text-[var(--tr-text-faint)]">Summe</span>{/if}
        </Label>
        <Input
          id="erz"
          type="number"
          step="0.1"
          min="0"
          bind:value={erzeugung}
          placeholder="0"
        />
      </div>
      <div class="space-y-1.5">
        <Label for="ei">
          Einspeisung (kWh)
          {#if mode !== "tag"}<span class="text-[var(--tr-text-faint)]">Summe</span>{/if}
        </Label>
        <Input
          id="ei"
          type="number"
          step="0.1"
          min="0"
          bind:value={einspeisung}
          placeholder="0"
        />
      </div>
      <div class="space-y-1.5">
        <Label for="ev">
          Eigenverbrauch (kWh)
          <span class="text-[var(--tr-text-faint)]">berechnet</span>
        </Label>
        <Input
          id="ev"
          type="number"
          step="0.1"
          readonly
          tabindex={-1}
          value={eigenverbrauchComputed}
          class="bg-[var(--tr-surface2)] text-[var(--tr-text-dim)]"
        />
      </div>
      <div class="space-y-1.5">
        <Label for="nb">
          Netzbezug (kWh)
          <span class="text-[var(--tr-text-faint)]">optional</span>
        </Label>
        <Input
          id="nb"
          type="number"
          step="0.1"
          min="0"
          bind:value={netzbezug}
          placeholder="—"
        />
      </div>
      <div class="space-y-1.5">
        <Label for="spl">
          Speicher Laden (kWh)
          <span class="text-[var(--tr-text-faint)]">Solar → Akku</span>
        </Label>
        <Input
          id="spl"
          type="number"
          step="0.1"
          min="0"
          bind:value={speicherLaden}
          placeholder="—"
        />
      </div>
      <div class="space-y-1.5">
        <Label for="spe">
          Speicher Entladen (kWh)
          <span class="text-[var(--tr-text-faint)]">Akku → Haus</span>
        </Label>
        <Input
          id="spe"
          type="number"
          step="0.1"
          min="0"
          bind:value={speicherEntladen}
          placeholder="—"
        />
      </div>
      <div class="space-y-1.5 md:col-span-4">
        <Label for="notiz">Notiz (optional)</Label>
        <Input
          id="notiz"
          bind:value={notiz}
          placeholder={mode === "tag"
            ? "z.B. Wetter, Wartung"
            : "wird auf alle Tage des Zeitraums geschrieben"}
        />
      </div>
      <div class="flex items-end gap-2">
        <Button variant="primary" onclick={save} disabled={busy}>
          <SaveIcon class="size-4" />
          Speichern
        </Button>
      </div>
    </div>

    {#if mode !== "tag" && periodDayCount > 0 && (Number(erzeugung) || Number(einspeisung))}
      <div
        class="border-t border-[var(--tr-line)] bg-[var(--tr-surface2)] px-5 py-2 text-xs text-[var(--tr-text-dim)]"
      >
        Verteilung pro Tag: Erzeugung
        {formatKWh(round1((Number(erzeugung) || 0) / periodDayCount))},
        Einspeisung
        {formatKWh(round1((Number(einspeisung) || 0) / periodDayCount))},
        Eigenverbrauch
        {formatKWh(round1(eigenverbrauchComputed / periodDayCount))}
        {#if existingInPeriod > 0}
          · {existingInPeriod} bestehende Tageseinträge werden überschrieben.
        {/if}
      </div>
    {/if}

    {#if eigenverbrauchNegativ}
      <div
        class="border-t border-[var(--tr-line)] bg-[var(--tr-warning-bg)] px-5 py-2 text-xs"
        style="color: var(--tr-warning);"
      >
        Hinweis: Einspeisung ist größer als Erzeugung — bitte prüfen.
      </div>
    {/if}
  </Card>

  {#if toast}
    <div
      class="fixed bottom-6 right-6 z-50 rounded-md border px-4 py-2 text-sm shadow"
      style:background={toast.kind === "ok"
        ? "var(--tr-green-bg)"
        : "var(--tr-red-bg)"}
      style:color={toast.kind === "ok" ? "var(--tr-green-dim)" : "var(--tr-red)"}
      style:border-color={toast.kind === "ok"
        ? "var(--tr-green)"
        : "var(--tr-red)"}
    >
      {toast.text}
    </div>
  {/if}

  <Card>
    <CardHeader
      title="Letzte 30 Tage"
      description="Klicke einen Tag, um ihn in die obere Maske zu laden."
    />
    {#if recent.length === 0}
      <div class="px-5 py-6 text-sm text-[var(--tr-text-dim)]">
        Noch keine Einträge.
      </div>
    {:else}
      <table class="w-full text-sm">
        <thead class="bg-[var(--tr-surface2)] text-xs uppercase text-[var(--tr-text-dim)]">
          <tr>
            <th class="px-5 py-2 text-left">Datum</th>
            <th class="px-5 py-2 text-right">Erzeugung</th>
            <th class="px-5 py-2 text-right">Eigenverbr.</th>
            <th class="px-5 py-2 text-right">Einspeisung</th>
            <th class="px-5 py-2 text-right" title="Solar → Akku">Sp ↓</th>
            <th class="px-5 py-2 text-right" title="Akku → Haus">Sp ↑</th>
            <th class="px-5 py-2 text-left">Notiz</th>
            <th class="px-5 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each recent as r (r.date)}
            <tr
              class="cursor-pointer border-t border-[var(--tr-line)] hover:bg-[var(--tr-surface2)]"
              onclick={() => selectRow(r.date)}
            >
              <td class="px-5 py-2 font-mono">{formatDateDE(r.date)}</td>
              <td class="px-5 py-2 text-right font-mono">
                {formatKWh(r.erzeugung_kwh)}
              </td>
              <td class="px-5 py-2 text-right font-mono">
                {formatKWh(r.eigenverbrauch_kwh)}
              </td>
              <td class="px-5 py-2 text-right font-mono">
                {formatKWh(r.einspeisung_kwh)}
              </td>
              <td class="px-5 py-2 text-right font-mono text-[var(--tr-text-dim)]">
                {r.speicher_laden_kwh != null ? formatKWh(r.speicher_laden_kwh) : "—"}
              </td>
              <td class="px-5 py-2 text-right font-mono text-[var(--tr-text-dim)]">
                {r.speicher_entladen_kwh != null ? formatKWh(r.speicher_entladen_kwh) : "—"}
              </td>
              <td class="px-5 py-2 text-[var(--tr-text-dim)]">{r.notiz ?? ""}</td>
              <td class="px-5 py-2 text-right">
                <button
                  type="button"
                  class="inline-flex h-7 w-7 items-center justify-center rounded-md text-[var(--tr-text-faint)] hover:bg-[var(--tr-red-bg)] hover:text-[var(--tr-red)]"
                  onclick={(e) => {
                    e.stopPropagation();
                    remove(r.date);
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
