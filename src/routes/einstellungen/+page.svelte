<script lang="ts">
  import { onMount } from "svelte";
  import { getSettings, setSettings } from "$lib/api";
  import type {
    BetreiberModus,
    BetreiberPeriode,
    EinspeiseModell,
    Settings,
    UstModus,
    UstPeriode,
    VerguetungPeriode,
  } from "$lib/types";
  import { todayISO, formatDateDE } from "$lib/utils";
  import Card from "$lib/components/ui/Card.svelte";
  import CardHeader from "$lib/components/ui/CardHeader.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Label from "$lib/components/ui/Label.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { PlusIcon, SaveIcon, Trash2Icon } from "@lucide/svelte";

  let settings = $state<Settings | null>(null);
  let error = $state<string | null>(null);
  let saved = $state(false);

  // Form-State für %-Eingabe statt 0.19
  let satzProzent = $state(19);
  let evPreis = $state(0.2);
  let bezugPreis = $state(0.35);
  let apiUrl = $state("");
  let apiToken = $state("");

  async function reload() {
    try {
      settings = await getSettings();
      satzProzent = settings.ust_satz_regel * 100;
      evPreis = settings.eigenverbrauch_preis;
      bezugPreis = settings.strom_bezugspreis;
      apiUrl = settings.anker_api_url ?? "";
      apiToken = settings.anker_api_token ?? "";
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(reload);

  function tempId(): number {
    return -Math.floor(Math.random() * 1e9);
  }

  function addUstPeriode() {
    if (!settings) return;
    const neu: UstPeriode = {
      id: tempId(),
      effective_from: todayISO(),
      modus: "regel",
    };
    settings.ust_perioden = [...settings.ust_perioden, neu].sort((a, b) =>
      a.effective_from.localeCompare(b.effective_from),
    );
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
      (a, b) => a.effective_from.localeCompare(b.effective_from),
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
      (a, b) => a.effective_from.localeCompare(b.effective_from),
    );
  }

  function removeVerguetungPeriode(id: number) {
    if (!settings) return;
    settings.verguetung_perioden = settings.verguetung_perioden.filter(
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
      settings.anker_api_url = apiUrl.trim() || null;
      settings.anker_api_token = apiToken.trim() || null;
      settings.ust_perioden = [...settings.ust_perioden].sort((a, b) =>
        a.effective_from.localeCompare(b.effective_from),
      );
      settings.betreiber_perioden = [...settings.betreiber_perioden].sort(
        (a, b) => a.effective_from.localeCompare(b.effective_from),
      );
      settings.verguetung_perioden = [...settings.verguetung_perioden].sort(
        (a, b) => a.effective_from.localeCompare(b.effective_from),
      );
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
    <Card>
      <CardHeader
        title="Betreiber-Status (ESt)"
        description="Privat = §3 Nr. 72 EStG (ESt-frei bis 30 kWp / 15 kWp je Wohneinheit). Gewerblich = EÜR-pflichtig."
      />
      <div class="divide-y divide-[var(--tr-line)]">
        {#each settings.betreiber_perioden as p (p.id)}
          <div class="grid grid-cols-1 items-end gap-3 px-5 py-3 md:grid-cols-3">
            <div class="space-y-1.5">
              <Label>Gültig ab</Label>
              <Input type="date" bind:value={p.effective_from} />
            </div>
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
            <div class="flex items-end">
              <Button variant="ghost" onclick={() => removeBetreiberPeriode(p.id)}>
                <Trash2Icon class="size-4" />Entfernen
              </Button>
            </div>
          </div>
        {/each}
      </div>
      <div class="border-t border-[var(--tr-line)] px-5 py-3">
        <Button variant="secondary" onclick={addBetreiberPeriode}>
          <PlusIcon class="size-4" />Periode hinzufügen
        </Button>
      </div>
    </Card>

    <Card>
      <CardHeader
        title="USt-Modus-Verlauf"
        description="Mehrere Perioden möglich (z.B. erst Regelbesteuerung, später Kleinunternehmer)."
      />
      <div class="divide-y divide-[var(--tr-line)]">
        {#each settings.ust_perioden as p (p.id)}
          <div class="grid grid-cols-1 items-end gap-3 px-5 py-3 md:grid-cols-3">
            <div class="space-y-1.5">
              <Label>Gültig ab</Label>
              <Input type="date" bind:value={p.effective_from} />
            </div>
            <div class="space-y-1.5">
              <Label>Modus</Label>
              <Select
                bind:value={p.modus as unknown as string}
                options={UST_MODI.map((m) => ({ value: m.value, label: m.label }))}
              />
            </div>
            <div class="flex items-end">
              <Button variant="ghost" onclick={() => removeUstPeriode(p.id)}>
                <Trash2Icon class="size-4" />Entfernen
              </Button>
            </div>
          </div>
        {/each}
      </div>
      <div class="border-t border-[var(--tr-line)] px-5 py-3">
        <Button variant="secondary" onclick={addUstPeriode}>
          <PlusIcon class="size-4" />Periode hinzufügen
        </Button>
      </div>
    </Card>

    <Card>
      <CardHeader
        title="Einspeisevergütung-Verlauf"
        description="EEG-Vergütungssätze je Inbetriebnahmemonat / Modell (ct/kWh). Wird für die Plausibilitätsprüfung der Auszahlungen verwendet."
      />
      <div class="divide-y divide-[var(--tr-line)]">
        {#each settings.verguetung_perioden as p (p.id)}
          <div class="grid grid-cols-1 items-end gap-3 px-5 py-3 md:grid-cols-4">
            <div class="space-y-1.5">
              <Label>Gültig ab</Label>
              <Input type="date" bind:value={p.effective_from} />
            </div>
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
            <div class="flex items-end">
              <Button variant="ghost" onclick={() => removeVerguetungPeriode(p.id)}>
                <Trash2Icon class="size-4" />Entfernen
              </Button>
            </div>
          </div>
        {/each}
        {#if settings.verguetung_perioden.length === 0}
          <div class="px-5 py-3 text-xs text-[var(--tr-text-dim)]">
            Noch kein Vergütungssatz hinterlegt. Ohne Eintrag ist keine
            Auszahlungs-Plausibilität möglich.
          </div>
        {/if}
      </div>
      <div class="border-t border-[var(--tr-line)] px-5 py-3">
        <Button variant="secondary" onclick={addVerguetungPeriode}>
          <PlusIcon class="size-4" />Periode hinzufügen
        </Button>
      </div>
    </Card>

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
          <Label>Strom-Bezugspreis (€ / kWh)</Label>
          <Input type="number" step="0.01" bind:value={bezugPreis} />
          <p class="text-xs text-[var(--tr-text-dim)]">
            Für die Ersparnis-Anzeige im Privatmodus (vermiedener Netzbezug).
          </p>
        </div>
      </div>
    </Card>

    <Card>
      <CardHeader
        title="Hersteller-API (Anker / Fronius / SMA …)"
        description="Optional. Wenn gesetzt, kann die Tageserfassung Werte importieren."
      />
      <div class="grid grid-cols-1 gap-4 px-5 py-5 md:grid-cols-2">
        <div class="space-y-1.5">
          <Label>API-URL</Label>
          <Input bind:value={apiUrl} placeholder="https://…/api/v1/production" />
        </div>
        <div class="space-y-1.5">
          <Label>API-Token</Label>
          <Input type="password" bind:value={apiToken} placeholder="••••••" />
        </div>
      </div>
      <div class="border-t border-[var(--tr-line)] px-5 py-3 text-xs text-[var(--tr-text-dim)]">
        Der konkrete Import-Adapter wird ergänzt, sobald die API-Spezifikation
        feststeht. Bis dahin meldet der Import-Button einen klaren Fehler statt
        stillschweigend nichts zu tun.
      </div>
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
  {/if}
</div>
