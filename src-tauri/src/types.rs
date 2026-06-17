//! Datentypen — spiegeln `src/lib/types.ts` feldweise.
//! Bei Änderungen beide Seiten synchron halten (kein Codegen).
//!
//! **Geldbeträge sind in Cents (`i64`).** Steuerberechnungen runden auf den
//! ganzen Cent. Rates / Preise pro Einheit (€/kWh, USt-Satz in Dezimal,
//! Sonderabschreibungs-%) bleiben `f64`, weil sie keine Geldbeträge sind und
//! kleine Dezimalstellen tragen.

use serde::{Deserialize, Serialize};

/// Rundet einen €-Berechnungs-Zwischenwert (z.B. `kwh × preis × satz`) auf
/// den ganzen Cent. Wird in Reports verwendet, wo Rates × Mengen entstehen.
pub fn round_to_cents(eur: f64) -> i64 {
    (eur * 100.0).round() as i64
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DailyProduction {
    pub date: String,
    pub erzeugung_kwh: f64,
    pub eigenverbrauch_kwh: f64,
    pub einspeisung_kwh: f64,
    #[serde(default)]
    pub netzbezug_kwh: Option<f64>,
    #[serde(default)]
    pub notiz: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Payout {
    pub id: i64,
    pub buchung_date: String,
    pub zeitraum_von: String,
    pub zeitraum_bis: String,
    /// Cents.
    pub netto: i64,
    /// Cents.
    pub ust: i64,
    /// Cents.
    pub brutto: i64,
    #[serde(default)]
    pub kwh: Option<f64>,
    #[serde(default)]
    pub notiz: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Expense {
    pub id: i64,
    pub date: String,
    pub kategorie: String,
    pub beschreibung: String,
    /// Cents.
    pub netto: i64,
    /// Cents.
    pub ust: i64,
    /// Cents.
    pub brutto: i64,
    pub vorsteuer_abzugsfaehig: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub inbetriebnahme: String,
    /// Cents.
    pub anschaffung_netto: i64,
    /// Cents.
    pub anschaffung_ust: i64,
    pub nutzungsdauer_jahre: i64,
    /// "linear" (default) | "gwg_sofort"
    #[serde(default = "default_afa_methode")]
    pub afa_methode: String,
    /// Sonderabschreibung §7g Abs. 5 EStG im Erstjahr (0..50 in Prozent der AK).
    #[serde(default)]
    pub sonderabschreibung_prozent: f64,
    #[serde(default)]
    pub verkauft_am: Option<String>,
    /// Cents.
    #[serde(default)]
    pub verkaufserloes_netto: Option<i64>,
    /// Cents.
    #[serde(default)]
    pub verkaufserloes_ust: Option<i64>,
    #[serde(default)]
    pub notiz: Option<String>,
}

fn default_afa_methode() -> String {
    "linear".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UstPeriode {
    pub id: i64,
    pub effective_from: String,
    /// "regel" | "kleinunternehmer" | "nullsteuer"
    pub modus: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BetreiberPeriode {
    pub id: i64,
    pub effective_from: String,
    /// "gewerblich" | "privat"
    pub modus: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerguetungPeriode {
    pub id: i64,
    pub effective_from: String,
    /// "ueberschuss" | "voll" | "direktvermarktung"
    pub modell: String,
    pub satz_ct_per_kwh: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StromtarifPeriode {
    pub id: i64,
    pub effective_from: String,
    /// Rate (€/kWh) — bleibt `f64`, ist kein Geldbetrag.
    pub arbeitspreis_eur_per_kwh: f64,
    /// Cents pro Monat.
    pub grundgebuehr_eur_per_monat: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub ust_perioden: Vec<UstPeriode>,
    pub betreiber_perioden: Vec<BetreiberPeriode>,
    pub verguetung_perioden: Vec<VerguetungPeriode>,
    pub stromtarif_perioden: Vec<StromtarifPeriode>,
    pub ust_satz_regel: f64,
    pub eigenverbrauch_preis: f64,
    /// Fallback-Arbeitspreis (€/kWh) — wenn kein Tarif-Eintrag für ein Datum existiert.
    pub strom_bezugspreis: f64,
    /// Aktiver Hersteller-API-Adapter: "none" | "anker" | "solaredge".
    /// Steuert welches Sidecar `import_from_vendor` aufruft.
    #[serde(default = "default_vendor")]
    pub vendor: String,
    #[serde(default)]
    pub anker_email: Option<String>,
    #[serde(default)]
    pub anker_password: Option<String>,
    #[serde(default = "default_country")]
    pub anker_country: String,
    #[serde(default)]
    pub solaredge_api_key: Option<String>,
    #[serde(default)]
    pub solaredge_site_id: Option<String>,
}

fn default_country() -> String {
    "DE".to_string()
}

fn default_vendor() -> String {
    "none".to_string()
}

#[derive(Serialize, Clone, Debug)]
pub struct Aggregat {
    pub bucket: String,
    pub erzeugung_kwh: f64,
    pub eigenverbrauch_kwh: f64,
    pub einspeisung_kwh: f64,
    pub tage: i64,
}

/// Alle Geldfelder in Cents.
#[derive(Serialize, Clone, Debug)]
pub struct EuerReport {
    pub jahr: i32,
    pub einnahmen_einspeisung_netto: i64,
    pub einnahmen_eigenverbrauch_netto: i64,
    pub einnahmen_veraeusserung_netto: i64,
    pub einnahmen_ust: i64,
    pub ausgaben_betrieb_netto: i64,
    pub ausgaben_betrieb_ust: i64,
    pub ausgaben_afa: i64,
    pub ausgaben_sonder_afa: i64,
    pub ausgaben_restbuchwert_abgang: i64,
    pub vorsteuer: i64,
    pub gewinn_vor_steuern: i64,
    pub betreiber_modus: String,
    pub est_pflichtig: bool,
    pub est_befreiungsgrund: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ExpectedEinspeisung {
    pub jahr: i32,
    pub monat: Option<i32>,
    pub kwh: f64,
    /// Cents.
    pub erwartet_netto: i64,
    /// Tage im Zeitraum, für die kein Vergütungssatz hinterlegt war.
    pub tage_ohne_satz: i64,
}

/// Alle Geldfelder in Cents.
#[derive(Serialize, Clone, Debug)]
pub struct UstvaReport {
    pub jahr: i32,
    pub monat: Option<i32>,
    pub modus: String,
    pub ust_einnahmen: i64,
    pub ust_eigenverbrauch: i64,
    pub vorsteuer: i64,
    pub zahllast: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct DashboardSnapshot {
    pub heute: Option<DailyProduction>,
    pub woche_kwh: f64,
    pub monat_kwh: f64,
    pub jahr_kwh: f64,
    pub max_tag: Option<DailyProduction>,
    /// Cents.
    pub einnahmen_jahr: i64,
    /// Eigenverbrauch im Jahr × Strom-Bezugspreis — vermiedene Stromkosten, Cents.
    pub einsparung_jahr: i64,
    pub betreiber_modus: String,
}

#[derive(Serialize)]
pub struct BackupSummary {
    pub daily: i64,
    pub payouts: i64,
    pub expenses: i64,
    pub assets: i64,
}
