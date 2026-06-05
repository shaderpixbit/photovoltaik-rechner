// Mirrors Rust structs in src-tauri/src/lib.rs. Keep field-for-field in sync.

export interface DailyProduction {
  /** ISO date YYYY-MM-DD */
  date: string;
  erzeugung_kwh: number;
  eigenverbrauch_kwh: number;
  einspeisung_kwh: number;
  /** Netzbezug (kWh aus dem Netz), optional — für Autarkiegrad / Privat-Ersparnis */
  netzbezug_kwh: number | null;
  notiz: string | null;
}

export interface Payout {
  id: number;
  /** Buchungsdatum / Gutschrift ISO */
  buchung_date: string;
  /** Abgerechneter Zeitraum von / bis (ISO) */
  zeitraum_von: string;
  zeitraum_bis: string;
  netto: number;
  ust: number;
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
  netto: number;
  ust: number;
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
  anschaffung_netto: number;
  anschaffung_ust: number;
  nutzungsdauer_jahre: number;
  afa_methode: AfaMethode;
  /** Sonderabschreibung §7g Abs. 5 EStG im Erstjahr (0–50 % der AK). */
  sonderabschreibung_prozent: number;
  verkauft_am: string | null;
  verkaufserloes_netto: number | null;
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
 * Einspeisemodell und Vergütungssatz (Cent/kWh). Modell:
 * - `ueberschuss` — Überschusseinspeisung (Standardfall bei Eigenverbrauch).
 * - `voll` — Volleinspeisung.
 * - `direktvermarktung` — Vermarktung über Direktvermarkter (variabler Satz,
 *   hier als Schnitt-/Anzahlungssatz erfasst).
 */
export type EinspeiseModell = "ueberschuss" | "voll" | "direktvermarktung";

export interface VerguetungPeriode {
  id: number;
  effective_from: string;
  modell: EinspeiseModell;
  satz_ct_per_kwh: number;
}

export interface Settings {
  ust_perioden: UstPeriode[];
  betreiber_perioden: BetreiberPeriode[];
  verguetung_perioden: VerguetungPeriode[];
  ust_satz_regel: number;
  eigenverbrauch_preis: number;
  /** Strom-Bezugspreis (€/kWh) für Ersparnis-Berechnung im Privatmodus. */
  strom_bezugspreis: number;
  anker_api_url: string | null;
  anker_api_token: string | null;
}

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
  erwartet_netto: number;
  /** Tage im Zeitraum mit Einspeisung, aber ohne hinterlegten Vergütungssatz. */
  tage_ohne_satz: number;
}

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
  einnahmen_jahr: number;
  /** Vermiedene Stromkosten = Σ Eigenverbrauch im Jahr × Strom-Bezugspreis. */
  einsparung_jahr: number;
  betreiber_modus: BetreiberModus;
}
