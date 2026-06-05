// Mirrors Rust structs in src-tauri/src/lib.rs. Keep field-for-field in sync.

export interface DailyProduction {
  /** ISO date YYYY-MM-DD */
  date: string;
  erzeugung_kwh: number;
  eigenverbrauch_kwh: number;
  einspeisung_kwh: number;
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

export interface Asset {
  id: number;
  name: string;
  inbetriebnahme: string;
  anschaffung_netto: number;
  anschaffung_ust: number;
  nutzungsdauer_jahre: number;
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

export interface Settings {
  ust_perioden: UstPeriode[];
  ust_satz_regel: number;
  eigenverbrauch_preis: number;
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
  einnahmen_ust: number;
  ausgaben_betrieb_netto: number;
  ausgaben_betrieb_ust: number;
  ausgaben_afa: number;
  vorsteuer: number;
  gewinn_vor_steuern: number;
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
}
