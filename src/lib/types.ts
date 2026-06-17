// Mirrors Rust structs in src-tauri/src/types.rs. Keep field-for-field in sync.
//
// Geldfelder sind in **Cents** (Integer). Rates wie €/kWh, USt-Sätze und
// Sonderabschreibung-% bleiben Dezimalzahlen. Formatter: `formatEUR(cents)`,
// Konvertierung Form ↔ API: `centsFromEuro(eur)` / `centsToEuro(cents)` aus utils.

export interface DailyProduction {
  /** ISO date YYYY-MM-DD */
  date: string;
  erzeugung_kwh: number;
  eigenverbrauch_kwh: number;
  einspeisung_kwh: number;
  /** Netzbezug (kWh aus dem Netz), optional — für Autarkiegrad / Privat-Ersparnis */
  netzbezug_kwh: number | null;
  /** Solar → Akku (kWh, kumuliert pro Tag). NULL bei reinen PV-Anlagen ohne Speicher. */
  speicher_laden_kwh: number | null;
  /** Akku → Haus (kWh). Pendant zu speicher_laden_kwh. */
  speicher_entladen_kwh: number | null;
  notiz: string | null;
}

export interface Payout {
  id: number;
  /** Buchungsdatum / Gutschrift ISO */
  buchung_date: string;
  /** Abgerechneter Zeitraum von / bis (ISO) */
  zeitraum_von: string;
  zeitraum_bis: string;
  /** Cents. */
  netto: number;
  /** Cents. */
  ust: number;
  /** Cents. */
  brutto: number;
  /** kWh laut Abrechnung (optional) */
  kwh: number | null;
  notiz: string | null;
}

export type ExpenseKategorie =
  | "Versicherung"
  | "Wartung"
  | "Reparatur"
  | "Zaehlermiete"
  | "Verwaltung"
  | "Sonstiges";

export interface Expense {
  id: number;
  date: string;
  kategorie: ExpenseKategorie;
  beschreibung: string;
  /** Cents. */
  netto: number;
  /** Cents. */
  ust: number;
  /** Cents. */
  brutto: number;
  vorsteuer_abzugsfaehig: boolean;
}

/**
 * AfA-Methode für ein Wirtschaftsgut:
 * - `linear` — lineare AfA über `nutzungsdauer_jahre`, pro-rata im Erstjahr.
 *   Bei Verkauf läuft die AfA bis zum Vormonat des Verkaufs.
 * - `gwg_sofort` — geringwertiges Wirtschaftsgut (§6 Abs. 2 EStG, netto ≤ 800 €).
 *   Komplette AK im Anschaffungsjahr als Aufwand, danach 0.
 */
export type AfaMethode = "linear" | "gwg_sofort";

export interface Asset {
  id: number;
  name: string;
  inbetriebnahme: string;
  /** Cents. */
  anschaffung_netto: number;
  /** Cents. */
  anschaffung_ust: number;
  nutzungsdauer_jahre: number;
  afa_methode: AfaMethode;
  /** Sonderabschreibung §7g Abs. 5 EStG im Erstjahr (0–50 % der AK). */
  sonderabschreibung_prozent: number;
  verkauft_am: string | null;
  /** Cents. */
  verkaufserloes_netto: number | null;
  /** Cents. */
  verkaufserloes_ust: number | null;
  notiz: string | null;
}

/**
 * Umsatzsteuer-Modus. Der Status kann zeitlich wechseln, daher ist `effective_from`
 * Pflicht. Bei „nullsteuer" entfällt §12(3) UStG seit 2023 die USt auf Anschaffung
 * und Eigenverbrauchsbesteuerung; Einspeisung ist trotzdem umsatzsteuerpflichtig
 * (regelbesteuert) wenn man nicht zusätzlich Kleinunternehmer ist.
 */
export type UstModus = "regel" | "kleinunternehmer" | "nullsteuer";

export interface UstPeriode {
  id: number;
  effective_from: string;
  modus: UstModus;
}

/**
 * Betreiber-Modus für die Einkommensteuer-Seite:
 * - `gewerblich` — voll EÜR-pflichtig.
 * - `privat` — PV-Anlage ≤30 kWp (bzw. 15 kWp je Wohneinheit) ist seit 2023
 *   einkommensteuerbefreit nach §3 Nr. 72 EStG. EÜR-Werte werden nur informativ
 *   ausgewiesen. Die UStVA-Seite bleibt davon unberührt — das ist eine andere
 *   Achse (siehe `UstModus`).
 */
export type BetreiberModus = "gewerblich" | "privat";

export interface BetreiberPeriode {
  id: number;
  effective_from: string;
  modus: BetreiberModus;
}

/**
 * Einspeisemodell und Vergütungssatz (Cent/kWh, bleibt Dezimal). Modell:
 * - `ueberschuss` — Überschusseinspeisung (Standardfall bei Eigenverbrauch).
 * - `voll` — Volleinspeisung.
 * - `direktvermarktung` — Vermarktung über Direktvermarkter.
 */
export type EinspeiseModell = "ueberschuss" | "voll" | "direktvermarktung";

export interface VerguetungPeriode {
  id: number;
  effective_from: string;
  modell: EinspeiseModell;
  satz_ct_per_kwh: number;
}

/**
 * Stromtarif-Verlauf für den Netzbezug. Wird für die Ersparnis-Berechnung
 * im Dashboard taggenau ausgewertet. Grundgebühr ist informativ (€/Monat),
 * fließt nicht automatisch in EÜR — gehört dort als separater Aufwand
 * gebucht.
 */
export interface StromtarifPeriode {
  id: number;
  effective_from: string;
  /** Rate €/kWh (Dezimal, kein Cent). */
  arbeitspreis_eur_per_kwh: number;
  /** Cents pro Monat. */
  grundgebuehr_eur_per_monat: number;
}

export interface Settings {
  ust_perioden: UstPeriode[];
  betreiber_perioden: BetreiberPeriode[];
  verguetung_perioden: VerguetungPeriode[];
  stromtarif_perioden: StromtarifPeriode[];
  ust_satz_regel: number;
  eigenverbrauch_preis: number;
  /** Fallback-Arbeitspreis (€/kWh) — greift wenn kein Tarif-Eintrag existiert. */
  strom_bezugspreis: number;
  /** Aktiver Hersteller-API-Adapter — steuert welches Sidecar der API-Import nutzt. */
  vendor: VendorKind;
  /** Anker-Cloud-Account fuer den Tagesdaten-Import. */
  anker_email: string | null;
  anker_password: string | null;
  /** ISO-Country-Code (DE, AT, CH …) — bestimmt EU- vs. COM-Endpoint. */
  anker_country: string;
  /** SolarEdge monitoring API Key (aus dem Admin-Bereich des Monitoring-Portals). */
  solaredge_api_key: string | null;
  /** SolarEdge Site-ID (numerisch, aus dem Monitoring-Portal). */
  solaredge_site_id: string | null;
}

/** Hersteller-API-Adapter:
 * - `none`     — kein API-Import, nur manuelle Erfassung
 * - `anker`    — Anker Solix Cloud (inoffiziell, via thomluther/anker-solix-api)
 * - `solaredge` — SolarEdge monitoringapi.solaredge.com (offiziell, REST + API-Key)
 */
export type VendorKind = "none" | "anker" | "solaredge";

/* ── Aggregations ─────────────────────────────────────────────────────────── */

export type Periode = "tag" | "monat" | "jahr" | "max";

export interface Aggregat {
  /** Bucket-Key (YYYY-MM-DD / YYYY-MM / YYYY / "gesamt") */
  bucket: string;
  erzeugung_kwh: number;
  eigenverbrauch_kwh: number;
  einspeisung_kwh: number;
  tage: number;
}

/** Alle Geldfelder in Cents. */
export interface EuerReport {
  jahr: number;
  einnahmen_einspeisung_netto: number;
  einnahmen_eigenverbrauch_netto: number;
  einnahmen_veraeusserung_netto: number;
  einnahmen_ust: number;
  ausgaben_betrieb_netto: number;
  ausgaben_betrieb_ust: number;
  ausgaben_afa: number;
  ausgaben_sonder_afa: number;
  ausgaben_restbuchwert_abgang: number;
  vorsteuer: number;
  gewinn_vor_steuern: number;
  betreiber_modus: BetreiberModus;
  est_pflichtig: boolean;
  est_befreiungsgrund: string | null;
}

export interface ExpectedEinspeisung {
  jahr: number;
  monat: number | null;
  kwh: number;
  /** Cents. */
  erwartet_netto: number;
  /** Tage im Zeitraum mit Einspeisung, aber ohne hinterlegten Vergütungssatz. */
  tage_ohne_satz: number;
}

/** Alle Geldfelder in Cents. */
export interface UstvaReport {
  jahr: number;
  monat: number | null;
  modus: UstModus;
  ust_einnahmen: number;
  ust_eigenverbrauch: number;
  vorsteuer: number;
  zahllast: number;
}

export interface DashboardSnapshot {
  heute: DailyProduction | null;
  woche_kwh: number;
  monat_kwh: number;
  jahr_kwh: number;
  max_tag: DailyProduction | null;
  /** Cents. */
  einnahmen_jahr: number;
  /** Cents. Vermiedene Stromkosten = Σ Eigenverbrauch × Strom-Bezugspreis. */
  einsparung_jahr: number;
  betreiber_modus: BetreiberModus;
}
