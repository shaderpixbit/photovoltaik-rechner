<script lang="ts">
  import { onMount } from "svelte";
  import {
    confirm as askConfirm,
    open as openDialog,
    save as saveDialog,
  } from "@tauri-apps/plugin-dialog";
  import {
    exportBackup,
    getSettings,
    importBackup,
    setSettings,
    wipeDatabase,
    type BackupSummary,
    type WipeSummary,
  } from "$lib/api";
  import type {
    BetreiberModus,
    BetreiberPeriode,
    EinspeiseModell,
    Settings,
    StromtarifPeriode,
    UstModus,
    UstPeriode,
    VendorKind,
    VerguetungPeriode,
  } from "$lib/types";
  import { centsToEuro, euroToCents, todayISO, formatDateDE } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import PeriodVerlauf from "$lib/components/PeriodVerlauf.svelte";
  import {
    DatabaseIcon,
    DownloadIcon,
    SaveIcon,
    TriangleAlertIcon,
    Trash2Icon,
    UploadIcon,
  } from "@lucide/svelte";

  let settings = $state<Settings | null>(null);
  let error = $state<string | null>(null);
  let saved = $state(false);

  // Form-State für %-Eingabe statt 0.19
  let satzProzent = $state(19);
  let evPreis = $state(0.2);
  let bezugPreis = $state(0.35);
  let vendor = $state<VendorKind>("none");
  let ankerEmail = $state("");
  let ankerPassword = $state("");
  let ankerCountry = $state("DE");
  let seApiKey = $state("");
  let seSiteId = $state("");

  async function reload() {
    try {
      settings = await getSettings();
      // Grundgebühr kommt in Cents — für die Form ins €-Darstellungs-Feld konvertieren.
      // Beim Save wird zurück konvertiert. Der Typ bleibt `number`, nur die Semantik
      // wechselt zwischen "API/Cents" und "Form/€".
      settings.stromtarif_perioden = settings.stromtarif_perioden.map((p) => ({
        ...p,
        grundgebuehr_eur_per_monat: centsToEuro(p.grundgebuehr_eur_per_monat),
      }));
      satzProzent = settings.ust_satz_regel * 100;
      evPreis = settings.eigenverbrauch_preis;
      bezugPreis = settings.strom_bezugspreis;
      vendor = settings.vendor || "none";
      ankerEmail = settings.anker_email ?? "";
      ankerPassword = settings.anker_password ?? "";
      ankerCountry = settings.anker_country || "DE";
      seApiKey = settings.solaredge_api_key ?? "";
      seSiteId = settings.solaredge_site_id ?? "";
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(reload);

  function tempId(): number {
    return -Math.floor(Math.random() * 1e9);
  }

  function byDate<T extends { effective_from: string }>(a: T, b: T): number {
    return a.effective_from.localeCompare(b.effective_from);
  }

  function addUstPeriode() {
    if (!settings) return;
    const neu: UstPeriode = {
      id: tempId(),
      effective_from: todayISO(),
      modus: "regel",
    };
    settings.ust_perioden = [...settings.ust_perioden, neu].sort(byDate);
  }

  function removeUstPeriode(id: number) {
    if (!settings) return;
    settings.ust_perioden = settings.ust_perioden.filter((p) => p.id !== id);
  }

  function addBetreiberPeriode() {
    if (!settings) return;
    const neu: BetreiberPeriode = {
      id: tempId(),
      effective_from: todayISO(),
      modus: "gewerblich",
    };
    settings.betreiber_perioden = [...settings.betreiber_perioden, neu].sort(
      byDate,
    );
  }

  function removeBetreiberPeriode(id: number) {
    if (!settings) return;
    settings.betreiber_perioden = settings.betreiber_perioden.filter(
      (p) => p.id !== id,
    );
  }

  function addVerguetungPeriode() {
    if (!settings) return;
    const neu: VerguetungPeriode = {
      id: tempId(),
      effective_from: todayISO(),
      modell: "ueberschuss",
      satz_ct_per_kwh: 8.2,
    };
    settings.verguetung_perioden = [...settings.verguetung_perioden, neu].sort(
      byDate,
    );
  }

  function removeVerguetungPeriode(id: number) {
    if (!settings) return;
    settings.verguetung_perioden = settings.verguetung_perioden.filter(
      (p) => p.id !== id,
    );
  }

  function addStromtarifPeriode() {
    if (!settings) return;
    const neu: StromtarifPeriode = {
      id: tempId(),
      effective_from: todayISO(),
      arbeitspreis_eur_per_kwh: bezugPreis || 0.35,
      grundgebuehr_eur_per_monat: 0,
    };
    settings.stromtarif_perioden = [...settings.stromtarif_perioden, neu].sort(
      byDate,
    );
  }

  function removeStromtarifPeriode(id: number) {
    if (!settings) return;
    settings.stromtarif_perioden = settings.stromtarif_perioden.filter(
      (p) => p.id !== id,
    );
  }

  async function save() {
    if (!settings) return;
    error = null;
    try {
      settings.ust_satz_regel = satzProzent / 100;
      settings.eigenverbrauch_preis = evPreis;
      settings.strom_bezugspreis = bezugPreis;
      settings.vendor = vendor;
      settings.anker_email = ankerEmail.trim() || null;
      settings.anker_password = ankerPassword.trim() || null;
      settings.anker_country = (ankerCountry.trim() || "DE").toUpperCase();
      settings.solaredge_api_key = seApiKey.trim() || null;
      settings.solaredge_site_id = seSiteId.trim() || null;
      settings.ust_perioden = [...settings.ust_perioden].sort(byDate);
      settings.betreiber_perioden = [...settings.betreiber_perioden].sort(byDate);
      settings.verguetung_perioden = [...settings.verguetung_perioden].sort(byDate);
      // Grundgebühr für API zurück in Cents konvertieren.
      settings.stromtarif_perioden = [...settings.stromtarif_perioden]
        .sort(byDate)
        .map((p) => ({
          ...p,
          grundgebuehr_eur_per_monat: euroToCents(p.grundgebuehr_eur_per_monat),
        }));
      await setSettings(settings);
      saved = true;
      setTimeout(() => (saved = false), 2000);
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  const UST_MODI: { value: UstModus; label: string }[] = [
    { value: "regel", label: "Regelbesteuerung 19%" },
    { value: "kleinunternehmer", label: "Kleinunternehmer §19 UStG" },
    { value: "nullsteuer", label: "Nullsteuersatz §12(3) UStG" },
  ];

  const BETREIBER_MODI: { value: BetreiberModus; label: string }[] = [
    { value: "gewerblich", label: "Gewerblich (EÜR-pflichtig)" },
    { value: "privat", label: "Privat (§3 Nr. 72 EStG, ESt-befreit)" },
  ];

  const EINSPEISE_MODELLE: { value: EinspeiseModell; label: string }[] = [
    { value: "ueberschuss", label: "Überschusseinspeisung" },
    { value: "voll", label: "Volleinspeisung" },
    { value: "direktvermarktung", label: "Direktvermarktung" },
  ];

  let backupMsg = $state<string | null>(null);
  let backupBusy = $state(false);

  function backupSummary(s: BackupSummary): string {
    return `Tage: ${s.daily} · Auszahlungen: ${s.payouts} · Ausgaben: ${s.expenses} · Anlagen: ${s.assets}`;
  }

  async function doExportBackup() {
    backupMsg = null;
    backupBusy = true;
    try {
      const path = await saveDialog({
        defaultPath: `photovoltaik-backup-${todayISO()}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      const summary = await exportBackup(path);
      backupMsg = `Backup exportiert. ${backupSummary(summary)}`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      backupBusy = false;
    }
  }

  async function doImportBackup() {
    const ok = await askConfirm(
      "Restore überschreibt ALLE bestehenden Daten dieser App (Tage, " +
        "Auszahlungen, Ausgaben, Anlagen, Verläufe, Einstellungen) mit dem " +
        "Inhalt der Backup-Datei. Fortfahren?",
      { title: "Backup wiederherstellen?", kind: "warning" },
    );
    if (!ok) return;
    backupMsg = null;
    backupBusy = true;
    try {
      const path = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path || typeof path !== "string") return;
      const summary = await importBackup(path);
      backupMsg = `Backup importiert. ${backupSummary(summary)}`;
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      backupBusy = false;
    }
  }

  // ── Gefahrenzone: kompletter DB-Wipe ──────────────────────────────────
  // Zwei-Stufen-Bestaetigung: Card aufklappen + WIPE eintippen + finaler Klick.
  let wipeOpen = $state(false);
  let wipeConfirmInput = $state("");
  let wipeBusy = $state(false);
  let wipeMsg = $state<string | null>(null);

  function wipeSummary(s: WipeSummary): string {
    return (
      `Geloescht: ${s.deleted_daily} Tage, ${s.deleted_payouts} Auszahlungen, ` +
      `${s.deleted_expenses} Ausgaben, ${s.deleted_assets} Anlagen, ` +
      `${s.deleted_verlauf_eintraege} Verlaufs-Eintraege.`
    );
  }

  async function doWipe() {
    if (wipeConfirmInput.trim() !== "WIPE") return;
    wipeBusy = true;
    wipeMsg = null;
    try {
      const summary = await wipeDatabase("WIPE");
      wipeMsg = wipeSummary(summary);
      wipeConfirmInput = "";
      wipeOpen = false;
      await reload();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      wipeBusy = false;
    }
  }
</script>

<div class="space-y-6">
  <div>
    <h1 class="text-2xl font-semibold tracking-tight">Einstellungen</h1>
    <p class="text-sm text-[var(--tr-text-dim)]">
      Betreiber-Status, USt-Modus, Vergütungssätze, Preise und API-Zugang.
    </p>
  </div>

  {#if error}
    <Card><div class="p-5 text-sm text-[var(--tr-red)]">{error}</div></Card>
  {/if}
  {#if !settings}
    <div class="text-sm text-[var(--tr-text-dim)]">Lädt…</div>
  {:else}
    <PeriodVerlauf
      title="Betreiber-Status (ESt)"
      description="Privat = §3 Nr. 72 EStG (ESt-frei bis 30 kWp / 15 kWp je Wohneinheit). Gewerblich = EÜR-pflichtig."
      items={settings.betreiber_perioden}
      onAdd={addBetreiberPeriode}
      onRemove={removeBetreiberPeriode}
    >
      {#snippet row(p: BetreiberPeriode)}
        <div class="space-y-1.5">
          <Label>Status</Label>
          <Select
            bind:value={p.modus as unknown as string}
            options={BETREIBER_MODI.map((m) => ({
              value: m.value,
              label: m.label,
            }))}
          />
        </div>
      {/snippet}
    </PeriodVerlauf>

    <PeriodVerlauf
      title="USt-Modus-Verlauf"
      description="Mehrere Perioden möglich (z.B. erst Regelbesteuerung, später Kleinunternehmer)."
      items={settings.ust_perioden}
      onAdd={addUstPeriode}
      onRemove={removeUstPeriode}
    >
      {#snippet row(p: UstPeriode)}
        <div class="space-y-1.5">
          <Label>Modus</Label>
          <Select
            bind:value={p.modus as unknown as string}
            options={UST_MODI.map((m) => ({ value: m.value, label: m.label }))}
          />
        </div>
      {/snippet}
    </PeriodVerlauf>

    <PeriodVerlauf
      title="Einspeisevergütung-Verlauf"
      description="EEG-Vergütungssätze je Inbetriebnahmemonat / Modell (ct/kWh). Wird für die Plausibilitätsprüfung der Auszahlungen verwendet."
      items={settings.verguetung_perioden}
      onAdd={addVerguetungPeriode}
      onRemove={removeVerguetungPeriode}
      columns={4}
      emptyMessage="Noch kein Vergütungssatz hinterlegt. Ohne Eintrag ist keine Auszahlungs-Plausibilität möglich."
    >
      {#snippet row(p: VerguetungPeriode)}
        <div class="space-y-1.5">
          <Label>Modell</Label>
          <Select
            bind:value={p.modell as unknown as string}
            options={EINSPEISE_MODELLE.map((m) => ({
              value: m.value,
              label: m.label,
            }))}
          />
        </div>
        <div class="space-y-1.5">
          <Label>Satz (ct / kWh)</Label>
          <Input type="number" step="0.001" bind:value={p.satz_ct_per_kwh} />
        </div>
      {/snippet}
    </PeriodVerlauf>

    <PeriodVerlauf
      title="Stromtarif-Verlauf"
      description="Arbeitspreis (€/kWh) und optional Grundgebühr (€/Monat). Wird für die Ersparnis-Berechnung im Dashboard taggenau ausgewertet."
      items={settings.stromtarif_perioden}
      onAdd={addStromtarifPeriode}
      onRemove={removeStromtarifPeriode}
      columns={4}
      emptyMessage="Kein Tarif hinterlegt — die Ersparnis fällt auf den Fallback-Preis unten zurück."
    >
      {#snippet row(p: StromtarifPeriode)}
        <div class="space-y-1.5">
          <Label>Arbeitspreis (€ / kWh)</Label>
          <Input
            type="number"
            step="0.0001"
            bind:value={p.arbeitspreis_eur_per_kwh}
          />
        </div>
        <div class="space-y-1.5">
          <Label>Grundgebühr (€ / Monat)</Label>
          <Input
            type="number"
            step="0.01"
            bind:value={p.grundgebuehr_eur_per_monat}
          />
        </div>
      {/snippet}
    </PeriodVerlauf>

    <Card>
      <CardHeader title="Steuersätze & Preise" />
      <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-3">
        <div class="space-y-1.5">
          <Label>USt-Satz Regelbesteuerung (%)</Label>
          <Input type="number" step="0.1" bind:value={satzProzent} />
        </div>
        <div class="space-y-1.5">
          <Label>Eigenverbrauchspreis (€ / kWh)</Label>
          <Input type="number" step="0.01" bind:value={evPreis} />
          <p class="text-xs text-[var(--tr-text-dim)]">
            Bewertung der unentgeltlichen Wertabgabe (Wiederbeschaffungspreis Strom).
          </p>
        </div>
        <div class="space-y-1.5">
          <Label>Strom-Bezugspreis (€ / kWh) — Fallback</Label>
          <Input type="number" step="0.01" bind:value={bezugPreis} />
          <p class="text-xs text-[var(--tr-text-dim)]">
            Greift nur, wenn kein Stromtarif-Eintrag oben hinterlegt ist.
          </p>
        </div>
      </div>
    </Card>

    <Card>
      <CardHeader
        title="Hersteller-API"
        description="Quelle für den Tagesdaten-Import auf /erfassung. Nur die Felder des gewählten Adapters werden verwendet."
      />
      <div class="px-5 pt-5">
        <Label>API-Adapter</Label>
        <Select
          bind:value={vendor as unknown as string}
          options={[
            { value: "none", label: "Keiner (nur manuell)" },
            { value: "anker", label: "Anker Solix Cloud (inoffiziell)" },
            { value: "solaredge", label: "SolarEdge (mySolarEdge, offiziell)" },
          ]}
        />
      </div>

      {#if vendor === "anker"}
        <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-3">
          <div class="space-y-1.5">
            <Label>Anker-Account-Email</Label>
            <Input
              type="email"
              bind:value={ankerEmail}
              placeholder="pv-readonly@beispiel.de"
              autocomplete="off"
            />
          </div>
          <div class="space-y-1.5">
            <Label>Anker-Passwort</Label>
            <Input
              type="password"
              bind:value={ankerPassword}
              placeholder="••••••"
              autocomplete="new-password"
            />
          </div>
          <div class="space-y-1.5">
            <Label>Land (ISO-Code)</Label>
            <Input
              bind:value={ankerCountry}
              placeholder="DE"
              maxlength={2}
            />
            <p class="text-xs text-[var(--tr-text-dim)]">
              DE/AT/CH → EU-Endpoint, US/etc. → Global-Endpoint.
            </p>
          </div>
        </div>
        <div
          class="border-t border-[var(--tr-line)] px-5 py-3 text-xs text-[var(--tr-text-dim)]"
        >
          Achtung: Anker erlaubt nur eine aktive Session pro Account — die Haupt-
          Handy-App fliegt sonst raus. Empfohlen: Zweit-Account anlegen und in
          der Anker-App als „Mitglied" zur Anlage einladen.
        </div>
      {:else if vendor === "solaredge"}
        <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-2">
          <div class="space-y-1.5">
            <Label>API-Key</Label>
            <Input
              type="password"
              bind:value={seApiKey}
              placeholder="••••••"
              autocomplete="off"
            />
            <p class="text-xs text-[var(--tr-text-dim)]">
              monitoring.solaredge.com → Admin → Site Access → API-Key.
            </p>
          </div>
          <div class="space-y-1.5">
            <Label>Site-ID</Label>
            <Input bind:value={seSiteId} placeholder="z. B. 1234567" />
            <p class="text-xs text-[var(--tr-text-dim)]">
              Sichtbar in der URL des Monitoring-Portals nach /site/.
            </p>
          </div>
        </div>
        <div
          class="border-t border-[var(--tr-line)] px-5 py-3 text-xs text-[var(--tr-text-dim)]"
        >
          Offizielle REST-API (max. 300 Requests / Tag). Eigenverbrauch /
          Einspeisung / Netzbezug nur verfügbar wenn ein Smart Meter oder
          Modbus-Energiezähler im SolarEdge-System gemeldet ist.
        </div>
      {:else}
        <div class="px-5 py-5 text-sm text-[var(--tr-text-dim)]">
          Kein Hersteller-API aktiv — der Import-Button auf /erfassung ist deaktiviert.
          Tagesdaten manuell erfassen oder einen Adapter auswählen.
        </div>
      {/if}
    </Card>

    <div class="flex items-center gap-3">
      <Button variant="primary" onclick={save}>
        <SaveIcon class="size-4" />Speichern
      </Button>
      {#if saved}
        <span class="text-sm" style="color: var(--tr-green-dim);">Gespeichert.</span>
      {/if}
    </div>

    <Card>
      <CardHeader
        title="Datensicherung"
        description="Vollständiger Export aller Daten als JSON. Restore überschreibt alles."
      />
      <div class="flex flex-wrap items-center gap-3 px-5 py-5">
        <Button variant="secondary" onclick={doExportBackup} disabled={backupBusy}>
          <DownloadIcon class="size-4" />Backup exportieren
        </Button>
        <Button variant="ghost" onclick={doImportBackup} disabled={backupBusy}>
          <UploadIcon class="size-4" />Backup importieren
        </Button>
        <span
          class="inline-flex items-center gap-2 text-xs text-[var(--tr-text-faint)]"
        >
          <DatabaseIcon class="size-3.5" />
          Datei: <code>photovoltaik.db</code> neben der Executable (WAL-Journal).
        </span>
      </div>
      {#if backupMsg}
        <div
          class="border-t border-[var(--tr-line)] px-5 py-2 text-sm"
          style="color: var(--tr-green-dim); background: var(--tr-green-bg);"
        >
          {backupMsg}
        </div>
      {/if}
    </Card>

    <!-- Gefahrenzone: irreversibler Komplett-Wipe. Bewusst rot eingefaerbt
         und mit Pflicht-Eingabe "WIPE" gegen versehentliches Ausloesen. -->
    <Card>
      <div
        class="flex items-center gap-2 border-b px-5 py-3"
        style="border-color: var(--tr-red); background: var(--tr-red-bg);"
      >
        <TriangleAlertIcon class="size-5" style="color: var(--tr-red);" />
        <div>
          <h3 class="text-sm font-semibold" style="color: var(--tr-red);">
            Gefahrenzone
          </h3>
          <p class="text-xs" style="color: var(--tr-red-dim, var(--tr-red));">
            Alle Daten dieser App unwiderruflich löschen. Nutze vorher
            „Backup exportieren".
          </p>
        </div>
      </div>

      {#if !wipeOpen}
        <div class="px-5 py-5">
          <button
            type="button"
            class="inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors hover:opacity-90"
            style="background: var(--tr-red); color: white;"
            onclick={() => {
              wipeOpen = true;
              wipeConfirmInput = "";
              wipeMsg = null;
            }}
          >
            <Trash2Icon class="size-4" />
            Datenbank komplett löschen…
          </button>
        </div>
      {:else}
        <div class="space-y-3 px-5 py-5">
          <p class="text-sm">
            Du bist dabei <strong>ALLE</strong> Tageserfassungen, Auszahlungen,
            Ausgaben, Anlagen, Verlaufstabellen und Einstellungen zu löschen.
            <strong>Diese Aktion kann nicht rückgängig gemacht werden.</strong>
          </p>
          <p class="text-sm">
            Tippe <code class="rounded px-1.5 py-0.5 font-mono text-xs"
              style="background: var(--tr-red-bg); color: var(--tr-red);">WIPE</code>
            ins Feld unten, um den Button freizuschalten:
          </p>
          <Input
            bind:value={wipeConfirmInput}
            placeholder="WIPE"
            autocomplete="off"
            spellcheck={false}
            class="font-mono"
          />
          <div class="flex flex-wrap items-center gap-2 pt-1">
            <button
              type="button"
              class="inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
              style="background: var(--tr-red); color: white;"
              disabled={wipeBusy || wipeConfirmInput.trim() !== "WIPE"}
              onclick={doWipe}
            >
              <Trash2Icon class="size-4" />
              {wipeBusy ? "Lösche…" : "Jetzt alles löschen"}
            </button>
            <Button
              variant="ghost"
              onclick={() => {
                wipeOpen = false;
                wipeConfirmInput = "";
              }}
              disabled={wipeBusy}
            >
              Abbrechen
            </Button>
          </div>
        </div>
      {/if}

      {#if wipeMsg}
        <div
          class="border-t px-5 py-2 text-sm"
          style="color: var(--tr-red); background: var(--tr-red-bg); border-color: var(--tr-red);"
        >
          {wipeMsg}
        </div>
      {/if}
    </Card>

    <Card>
      <CardHeader title="Aktuell hinterlegt — Betreiber-Status" />
      <ul class="divide-y divide-[var(--tr-line)] text-sm">
        {#each settings.betreiber_perioden as p (p.id)}
          <li class="flex items-center justify-between px-5 py-2">
            <span class="font-mono">ab {formatDateDE(p.effective_from)}</span>
            <span>
              {BETREIBER_MODI.find((m) => m.value === p.modus)?.label ?? p.modus}
            </span>
          </li>
        {/each}
      </ul>
    </Card>

    <Card>
      <CardHeader title="Aktuell hinterlegt — USt-Modus" />
      <ul class="divide-y divide-[var(--tr-line)] text-sm">
        {#each settings.ust_perioden as p (p.id)}
          <li class="flex items-center justify-between px-5 py-2">
            <span class="font-mono">ab {formatDateDE(p.effective_from)}</span>
            <span>{UST_MODI.find((m) => m.value === p.modus)?.label ?? p.modus}</span>
          </li>
        {/each}
      </ul>
    </Card>

    <Card>
      <CardHeader title="Aktuell hinterlegt — Vergütungssätze" />
      <ul class="divide-y divide-[var(--tr-line)] text-sm">
        {#each settings.verguetung_perioden as p (p.id)}
          <li class="flex items-center justify-between px-5 py-2">
            <span class="font-mono">ab {formatDateDE(p.effective_from)}</span>
            <span>
              {EINSPEISE_MODELLE.find((m) => m.value === p.modell)?.label ?? p.modell}
              — {p.satz_ct_per_kwh.toLocaleString("de-DE", {
                minimumFractionDigits: 2,
                maximumFractionDigits: 3,
              })} ct/kWh
            </span>
          </li>
        {:else}
          <li class="px-5 py-2 text-xs text-[var(--tr-text-dim)]">
            Keine Vergütungssätze hinterlegt.
          </li>
        {/each}
      </ul>
    </Card>

    <Card>
      <CardHeader title="Aktuell hinterlegt — Stromtarif" />
      <ul class="divide-y divide-[var(--tr-line)] text-sm">
        {#each settings.stromtarif_perioden as p (p.id)}
          <li class="flex items-center justify-between px-5 py-2">
            <span class="font-mono">ab {formatDateDE(p.effective_from)}</span>
            <span>
              {p.arbeitspreis_eur_per_kwh.toLocaleString("de-DE", {
                minimumFractionDigits: 2,
                maximumFractionDigits: 4,
              })} €/kWh
              {#if p.grundgebuehr_eur_per_monat > 0}
                · {p.grundgebuehr_eur_per_monat.toLocaleString("de-DE", {
                  minimumFractionDigits: 2,
                  maximumFractionDigits: 2,
                })} €/Monat Grundgebühr
              {/if}
            </span>
          </li>
        {:else}
          <li class="px-5 py-2 text-xs text-[var(--tr-text-dim)]">
            Kein Stromtarif hinterlegt.
          </li>
        {/each}
      </ul>
    </Card>
  {/if}
</div>
