use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{command, State};

// ── Shared DB state ──────────────────────────────────────────────────────────

pub struct DbState(pub Mutex<Connection>);

// ── Data types (mirror src/lib/types.ts field-for-field) ────────────────────

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
    pub netto: f64,
    pub ust: f64,
    pub brutto: f64,
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
    pub netto: f64,
    pub ust: f64,
    pub brutto: f64,
    pub vorsteuer_abzugsfaehig: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub inbetriebnahme: String,
    pub anschaffung_netto: f64,
    pub anschaffung_ust: f64,
    pub nutzungsdauer_jahre: i64,
    /// "linear" (default) | "gwg_sofort"
    #[serde(default = "default_afa_methode")]
    pub afa_methode: String,
    /// Sonderabschreibung §7g Abs. 5 EStG im Erstjahr (0..50 in Prozent der AK).
    #[serde(default)]
    pub sonderabschreibung_prozent: f64,
    #[serde(default)]
    pub verkauft_am: Option<String>,
    #[serde(default)]
    pub verkaufserloes_netto: Option<f64>,
    #[serde(default)]
    pub verkaufserloes_ust: Option<f64>,
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
    pub arbeitspreis_eur_per_kwh: f64,
    pub grundgebuehr_eur_per_monat: f64,
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
    #[serde(default)]
    pub anker_api_url: Option<String>,
    #[serde(default)]
    pub anker_api_token: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Aggregat {
    pub bucket: String,
    pub erzeugung_kwh: f64,
    pub eigenverbrauch_kwh: f64,
    pub einspeisung_kwh: f64,
    pub tage: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct EuerReport {
    pub jahr: i32,
    pub einnahmen_einspeisung_netto: f64,
    pub einnahmen_eigenverbrauch_netto: f64,
    pub einnahmen_veraeusserung_netto: f64,
    pub einnahmen_ust: f64,
    pub ausgaben_betrieb_netto: f64,
    pub ausgaben_betrieb_ust: f64,
    pub ausgaben_afa: f64,
    pub ausgaben_sonder_afa: f64,
    pub ausgaben_restbuchwert_abgang: f64,
    pub vorsteuer: f64,
    pub gewinn_vor_steuern: f64,
    pub betreiber_modus: String,
    pub est_pflichtig: bool,
    pub est_befreiungsgrund: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ExpectedEinspeisung {
    pub jahr: i32,
    pub monat: Option<i32>,
    pub kwh: f64,
    pub erwartet_netto: f64,
    /// Tage im Zeitraum, für die kein Vergütungssatz hinterlegt war.
    pub tage_ohne_satz: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct UstvaReport {
    pub jahr: i32,
    pub monat: Option<i32>,
    pub modus: String,
    pub ust_einnahmen: f64,
    pub ust_eigenverbrauch: f64,
    pub vorsteuer: f64,
    pub zahllast: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct DashboardSnapshot {
    pub heute: Option<DailyProduction>,
    pub woche_kwh: f64,
    pub monat_kwh: f64,
    pub jahr_kwh: f64,
    pub max_tag: Option<DailyProduction>,
    pub einnahmen_jahr: f64,
    /// Eigenverbrauch im Jahr × Strom-Bezugspreis (€) — vermiedene Stromkosten.
    pub einsparung_jahr: f64,
    pub betreiber_modus: String,
}

// ── DB path & connection ────────────────────────────────────────────────────

fn get_db_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .or_else(|_| std::env::current_dir())
        .expect("Cannot determine path");
    path.pop();
    path.push("photovoltaik.db");
    path
}

pub fn open_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(get_db_path())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=15000;
         PRAGMA foreign_keys=ON;
         PRAGMA temp_store=MEMORY;",
    )?;
    create_schema(&conn)?;
    seed_defaults(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily_production (
            date              TEXT PRIMARY KEY,
            erzeugung_kwh     REAL NOT NULL DEFAULT 0,
            eigenverbrauch_kwh REAL NOT NULL DEFAULT 0,
            einspeisung_kwh   REAL NOT NULL DEFAULT 0,
            notiz             TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_daily_year ON daily_production(substr(date,1,4));
         CREATE INDEX IF NOT EXISTS idx_daily_month ON daily_production(substr(date,1,7));

         CREATE TABLE IF NOT EXISTS payouts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            buchung_date    TEXT NOT NULL,
            zeitraum_von    TEXT NOT NULL,
            zeitraum_bis    TEXT NOT NULL,
            netto           REAL NOT NULL DEFAULT 0,
            ust             REAL NOT NULL DEFAULT 0,
            brutto          REAL NOT NULL DEFAULT 0,
            kwh             REAL,
            notiz           TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_payouts_year ON payouts(substr(buchung_date,1,4));

         CREATE TABLE IF NOT EXISTS expenses (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            date            TEXT NOT NULL,
            kategorie       TEXT NOT NULL,
            beschreibung    TEXT NOT NULL DEFAULT '',
            netto           REAL NOT NULL DEFAULT 0,
            ust             REAL NOT NULL DEFAULT 0,
            brutto          REAL NOT NULL DEFAULT 0,
            vorsteuer_abzugsfaehig INTEGER NOT NULL DEFAULT 1
         );
         CREATE INDEX IF NOT EXISTS idx_expenses_year ON expenses(substr(date,1,4));

         CREATE TABLE IF NOT EXISTS assets (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            name               TEXT NOT NULL,
            inbetriebnahme     TEXT NOT NULL,
            anschaffung_netto  REAL NOT NULL DEFAULT 0,
            anschaffung_ust    REAL NOT NULL DEFAULT 0,
            nutzungsdauer_jahre INTEGER NOT NULL DEFAULT 20,
            notiz              TEXT
         );

         CREATE TABLE IF NOT EXISTS ust_perioden (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            effective_from  TEXT NOT NULL,
            modus           TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS betreiber_perioden (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            effective_from  TEXT NOT NULL,
            modus           TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS verguetung_perioden (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            effective_from      TEXT NOT NULL,
            modell              TEXT NOT NULL,
            satz_ct_per_kwh     REAL NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS stromtarif_perioden (
            id                          INTEGER PRIMARY KEY AUTOINCREMENT,
            effective_from              TEXT NOT NULL,
            arbeitspreis_eur_per_kwh    REAL NOT NULL DEFAULT 0,
            grundgebuehr_eur_per_monat  REAL NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
         );",
    )?;
    add_column_if_missing(conn, "daily_production", "netzbezug_kwh", "REAL")?;
    add_column_if_missing(
        conn,
        "assets",
        "afa_methode",
        "TEXT NOT NULL DEFAULT 'linear'",
    )?;
    add_column_if_missing(
        conn,
        "assets",
        "sonderabschreibung_prozent",
        "REAL NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "assets", "verkauft_am", "TEXT")?;
    add_column_if_missing(conn, "assets", "verkaufserloes_netto", "REAL")?;
    add_column_if_missing(conn, "assets", "verkaufserloes_ust", "REAL")?;
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let exists = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == column);
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, decl),
            [],
        )?;
    }
    Ok(())
}

fn seed_defaults(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('ust_satz_regel', '0.19')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('eigenverbrauch_preis', '0.20')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('anker_api_url', '')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('anker_api_token', '')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('strom_bezugspreis', '0.35')",
        [],
    )?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM ust_perioden", [], |r| r.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO ust_perioden (effective_from, modus) VALUES ('2000-01-01', 'regel')",
            [],
        )?;
    }
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM betreiber_perioden", [], |r| r.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO betreiber_perioden (effective_from, modus)
             VALUES ('2000-01-01', 'gewerblich')",
            [],
        )?;
    }
    Ok(())
}

// ── Settings helpers ────────────────────────────────────────────────────────

fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
}

fn get_setting_f64(conn: &Connection, key: &str, default: f64) -> f64 {
    get_setting(conn, key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}

fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn load_perioden(conn: &Connection) -> Result<Vec<UstPeriode>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, effective_from, modus FROM ust_perioden ORDER BY effective_from ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(UstPeriode {
                id: r.get(0)?,
                effective_from: r.get(1)?,
                modus: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn load_betreiber_perioden(conn: &Connection) -> Result<Vec<BetreiberPeriode>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, effective_from, modus FROM betreiber_perioden ORDER BY effective_from ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(BetreiberPeriode {
                id: r.get(0)?,
                effective_from: r.get(1)?,
                modus: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn load_verguetung_perioden(conn: &Connection) -> Result<Vec<VerguetungPeriode>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, effective_from, modell, satz_ct_per_kwh FROM verguetung_perioden
         ORDER BY effective_from ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(VerguetungPeriode {
                id: r.get(0)?,
                effective_from: r.get(1)?,
                modell: r.get(2)?,
                satz_ct_per_kwh: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn load_stromtarif_perioden(conn: &Connection) -> Result<Vec<StromtarifPeriode>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, effective_from, arbeitspreis_eur_per_kwh, grundgebuehr_eur_per_monat
         FROM stromtarif_perioden ORDER BY effective_from ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(StromtarifPeriode {
                id: r.get(0)?,
                effective_from: r.get(1)?,
                arbeitspreis_eur_per_kwh: r.get(2)?,
                grundgebuehr_eur_per_monat: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Liefert Arbeitspreis (€/kWh) und Grundgebühr (€/Monat) am Stichtag.
/// Fällt auf das Setting `strom_bezugspreis` zurück, wenn kein Eintrag existiert.
fn stromtarif_for(
    perioden: &[StromtarifPeriode],
    date: &str,
    fallback_arbeitspreis: f64,
) -> (f64, f64) {
    let mut arbeit = fallback_arbeitspreis;
    let mut grund = 0.0;
    let mut hit = false;
    for p in perioden {
        if p.effective_from.as_str() <= date {
            arbeit = p.arbeitspreis_eur_per_kwh;
            grund = p.grundgebuehr_eur_per_monat;
            hit = true;
        }
    }
    if !hit {
        (fallback_arbeitspreis, 0.0)
    } else {
        (arbeit, grund)
    }
}

/// Picks the modus active on `date` (the latest period whose effective_from ≤ date).
fn modus_for(perioden: &[UstPeriode], date: &str) -> String {
    let mut chosen = "regel".to_string();
    for p in perioden {
        if p.effective_from.as_str() <= date {
            chosen = p.modus.clone();
        }
    }
    chosen
}

fn betreiber_modus_for(perioden: &[BetreiberPeriode], date: &str) -> String {
    let mut chosen = "gewerblich".to_string();
    for p in perioden {
        if p.effective_from.as_str() <= date {
            chosen = p.modus.clone();
        }
    }
    chosen
}

/// Returns the vergütungs-satz (€/kWh) and modell active on `date`, or None if
/// no period covers it (e.g. before the first effective_from).
fn verguetung_for(perioden: &[VerguetungPeriode], date: &str) -> Option<(f64, String)> {
    let mut chosen: Option<(f64, String)> = None;
    for p in perioden {
        if p.effective_from.as_str() <= date {
            chosen = Some((p.satz_ct_per_kwh / 100.0, p.modell.clone()));
        }
    }
    chosen
}

// ── Tageserfassung ──────────────────────────────────────────────────────────

#[command]
fn list_daily_range(
    state: State<DbState>,
    from: String,
    to: String,
) -> Result<Vec<DailyProduction>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz
             FROM daily_production
             WHERE date BETWEEN ?1 AND ?2
             ORDER BY date ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<DailyProduction> = stmt
        .query_map(params![from, to], |r| {
            Ok(DailyProduction {
                date: r.get(0)?,
                erzeugung_kwh: r.get(1)?,
                eigenverbrauch_kwh: r.get(2)?,
                einspeisung_kwh: r.get(3)?,
                netzbezug_kwh: r.get(4)?,
                notiz: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[command]
fn get_daily(state: State<DbState>, date: String) -> Result<Option<DailyProduction>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.query_row(
        "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz
         FROM daily_production WHERE date = ?1",
        params![date],
        |r| {
            Ok(DailyProduction {
                date: r.get(0)?,
                erzeugung_kwh: r.get(1)?,
                eigenverbrauch_kwh: r.get(2)?,
                einspeisung_kwh: r.get(3)?,
                netzbezug_kwh: r.get(4)?,
                notiz: r.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[command]
fn upsert_daily(state: State<DbState>, entry: DailyProduction) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO daily_production
         (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(date) DO UPDATE SET
            erzeugung_kwh = excluded.erzeugung_kwh,
            eigenverbrauch_kwh = excluded.eigenverbrauch_kwh,
            einspeisung_kwh = excluded.einspeisung_kwh,
            netzbezug_kwh = excluded.netzbezug_kwh,
            notiz = excluded.notiz",
        params![
            entry.date,
            entry.erzeugung_kwh,
            entry.eigenverbrauch_kwh,
            entry.einspeisung_kwh,
            entry.netzbezug_kwh,
            entry.notiz
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
fn delete_daily(state: State<DbState>, date: String) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM daily_production WHERE date = ?1", params![date])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Payouts ─────────────────────────────────────────────────────────────────

#[command]
fn list_payouts(state: State<DbState>) -> Result<Vec<Payout>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT id, buchung_date, zeitraum_von, zeitraum_bis,
                    netto, ust, brutto, kwh, notiz
             FROM payouts ORDER BY buchung_date DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<Payout> = stmt
        .query_map([], |r| {
            Ok(Payout {
                id: r.get(0)?,
                buchung_date: r.get(1)?,
                zeitraum_von: r.get(2)?,
                zeitraum_bis: r.get(3)?,
                netto: r.get(4)?,
                ust: r.get(5)?,
                brutto: r.get(6)?,
                kwh: r.get(7)?,
                notiz: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[command]
fn upsert_payout(state: State<DbState>, payout: Payout) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    if payout.id > 0 {
        db.execute(
            "UPDATE payouts SET
                buchung_date = ?1, zeitraum_von = ?2, zeitraum_bis = ?3,
                netto = ?4, ust = ?5, brutto = ?6, kwh = ?7, notiz = ?8
             WHERE id = ?9",
            params![
                payout.buchung_date,
                payout.zeitraum_von,
                payout.zeitraum_bis,
                payout.netto,
                payout.ust,
                payout.brutto,
                payout.kwh,
                payout.notiz,
                payout.id
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(payout.id)
    } else {
        db.execute(
            "INSERT INTO payouts
             (buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto, kwh, notiz)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                payout.buchung_date,
                payout.zeitraum_von,
                payout.zeitraum_bis,
                payout.netto,
                payout.ust,
                payout.brutto,
                payout.kwh,
                payout.notiz
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(db.last_insert_rowid())
    }
}

#[command]
fn delete_payout(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM payouts WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Expenses ────────────────────────────────────────────────────────────────

#[command]
fn list_expenses(state: State<DbState>) -> Result<Vec<Expense>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT id, date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig
             FROM expenses ORDER BY date DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<Expense> = stmt
        .query_map([], |r| {
            Ok(Expense {
                id: r.get(0)?,
                date: r.get(1)?,
                kategorie: r.get(2)?,
                beschreibung: r.get(3)?,
                netto: r.get(4)?,
                ust: r.get(5)?,
                brutto: r.get(6)?,
                vorsteuer_abzugsfaehig: r.get::<_, i64>(7)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[command]
fn upsert_expense(state: State<DbState>, expense: Expense) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let vsa: i64 = if expense.vorsteuer_abzugsfaehig { 1 } else { 0 };
    if expense.id > 0 {
        db.execute(
            "UPDATE expenses SET
                date = ?1, kategorie = ?2, beschreibung = ?3,
                netto = ?4, ust = ?5, brutto = ?6, vorsteuer_abzugsfaehig = ?7
             WHERE id = ?8",
            params![
                expense.date,
                expense.kategorie,
                expense.beschreibung,
                expense.netto,
                expense.ust,
                expense.brutto,
                vsa,
                expense.id
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(expense.id)
    } else {
        db.execute(
            "INSERT INTO expenses
             (date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                expense.date,
                expense.kategorie,
                expense.beschreibung,
                expense.netto,
                expense.ust,
                expense.brutto,
                vsa
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(db.last_insert_rowid())
    }
}

#[command]
fn delete_expense(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM expenses WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Assets / AfA ────────────────────────────────────────────────────────────

const ASSET_COLS: &str = "id, name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
    nutzungsdauer_jahre, afa_methode, sonderabschreibung_prozent,
    verkauft_am, verkaufserloes_netto, verkaufserloes_ust, notiz";

fn map_asset(r: &rusqlite::Row) -> rusqlite::Result<Asset> {
    Ok(Asset {
        id: r.get(0)?,
        name: r.get(1)?,
        inbetriebnahme: r.get(2)?,
        anschaffung_netto: r.get(3)?,
        anschaffung_ust: r.get(4)?,
        nutzungsdauer_jahre: r.get(5)?,
        afa_methode: r.get(6)?,
        sonderabschreibung_prozent: r.get(7)?,
        verkauft_am: r.get(8)?,
        verkaufserloes_netto: r.get(9)?,
        verkaufserloes_ust: r.get(10)?,
        notiz: r.get(11)?,
    })
}

#[command]
fn list_assets(state: State<DbState>) -> Result<Vec<Asset>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {ASSET_COLS} FROM assets ORDER BY inbetriebnahme ASC, id ASC"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let rows: Vec<Asset> = stmt
        .query_map([], map_asset)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[command]
fn upsert_asset(state: State<DbState>, asset: Asset) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    if asset.id > 0 {
        db.execute(
            "UPDATE assets SET
                name = ?1, inbetriebnahme = ?2, anschaffung_netto = ?3,
                anschaffung_ust = ?4, nutzungsdauer_jahre = ?5,
                afa_methode = ?6, sonderabschreibung_prozent = ?7,
                verkauft_am = ?8, verkaufserloes_netto = ?9, verkaufserloes_ust = ?10,
                notiz = ?11
             WHERE id = ?12",
            params![
                asset.name,
                asset.inbetriebnahme,
                asset.anschaffung_netto,
                asset.anschaffung_ust,
                asset.nutzungsdauer_jahre,
                asset.afa_methode,
                asset.sonderabschreibung_prozent,
                asset.verkauft_am,
                asset.verkaufserloes_netto,
                asset.verkaufserloes_ust,
                asset.notiz,
                asset.id
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(asset.id)
    } else {
        db.execute(
            "INSERT INTO assets
             (name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
              nutzungsdauer_jahre, afa_methode, sonderabschreibung_prozent,
              verkauft_am, verkaufserloes_netto, verkaufserloes_ust, notiz)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                asset.name,
                asset.inbetriebnahme,
                asset.anschaffung_netto,
                asset.anschaffung_ust,
                asset.nutzungsdauer_jahre,
                asset.afa_methode,
                asset.sonderabschreibung_prozent,
                asset.verkauft_am,
                asset.verkaufserloes_netto,
                asset.verkaufserloes_ust,
                asset.notiz
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(db.last_insert_rowid())
    }
}

#[command]
fn delete_asset(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM assets WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Settings ────────────────────────────────────────────────────────────────

#[command]
fn get_settings(state: State<DbState>) -> Result<Settings, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let ust_perioden = load_perioden(&db).map_err(|e| e.to_string())?;
    let betreiber_perioden = load_betreiber_perioden(&db).map_err(|e| e.to_string())?;
    let verguetung_perioden = load_verguetung_perioden(&db).map_err(|e| e.to_string())?;
    let stromtarif_perioden = load_stromtarif_perioden(&db).map_err(|e| e.to_string())?;
    let url = get_setting(&db, "anker_api_url")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    let token = get_setting(&db, "anker_api_token")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    Ok(Settings {
        ust_perioden,
        betreiber_perioden,
        verguetung_perioden,
        stromtarif_perioden,
        ust_satz_regel: get_setting_f64(&db, "ust_satz_regel", 0.19),
        eigenverbrauch_preis: get_setting_f64(&db, "eigenverbrauch_preis", 0.20),
        strom_bezugspreis: get_setting_f64(&db, "strom_bezugspreis", 0.35),
        anker_api_url: url,
        anker_api_token: token,
    })
}

#[command]
fn set_settings(state: State<DbState>, settings: Settings) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    set_setting(&db, "ust_satz_regel", &settings.ust_satz_regel.to_string())
        .map_err(|e| e.to_string())?;
    set_setting(
        &db,
        "eigenverbrauch_preis",
        &settings.eigenverbrauch_preis.to_string(),
    )
    .map_err(|e| e.to_string())?;
    set_setting(
        &db,
        "strom_bezugspreis",
        &settings.strom_bezugspreis.to_string(),
    )
    .map_err(|e| e.to_string())?;
    set_setting(
        &db,
        "anker_api_url",
        settings.anker_api_url.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;
    set_setting(
        &db,
        "anker_api_token",
        settings.anker_api_token.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;

    db.execute("DELETE FROM ust_perioden", [])
        .map_err(|e| e.to_string())?;
    for p in &settings.ust_perioden {
        db.execute(
            "INSERT INTO ust_perioden (effective_from, modus) VALUES (?1, ?2)",
            params![p.effective_from, p.modus],
        )
        .map_err(|e| e.to_string())?;
    }
    if settings.ust_perioden.is_empty() {
        db.execute(
            "INSERT INTO ust_perioden (effective_from, modus) VALUES ('2000-01-01', 'regel')",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    db.execute("DELETE FROM betreiber_perioden", [])
        .map_err(|e| e.to_string())?;
    for p in &settings.betreiber_perioden {
        db.execute(
            "INSERT INTO betreiber_perioden (effective_from, modus) VALUES (?1, ?2)",
            params![p.effective_from, p.modus],
        )
        .map_err(|e| e.to_string())?;
    }
    if settings.betreiber_perioden.is_empty() {
        db.execute(
            "INSERT INTO betreiber_perioden (effective_from, modus)
             VALUES ('2000-01-01', 'gewerblich')",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    db.execute("DELETE FROM verguetung_perioden", [])
        .map_err(|e| e.to_string())?;
    for p in &settings.verguetung_perioden {
        db.execute(
            "INSERT INTO verguetung_perioden (effective_from, modell, satz_ct_per_kwh)
             VALUES (?1, ?2, ?3)",
            params![p.effective_from, p.modell, p.satz_ct_per_kwh],
        )
        .map_err(|e| e.to_string())?;
    }

    db.execute("DELETE FROM stromtarif_perioden", [])
        .map_err(|e| e.to_string())?;
    for p in &settings.stromtarif_perioden {
        db.execute(
            "INSERT INTO stromtarif_perioden
             (effective_from, arbeitspreis_eur_per_kwh, grundgebuehr_eur_per_monat)
             VALUES (?1, ?2, ?3)",
            params![
                p.effective_from,
                p.arbeitspreis_eur_per_kwh,
                p.grundgebuehr_eur_per_monat
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Aggregations ────────────────────────────────────────────────────────────

#[command]
fn aggregate_production(
    state: State<DbState>,
    periode: String,
    jahr: Option<i32>,
) -> Result<Vec<Aggregat>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let (bucket_expr, where_clause, params_vec): (&str, String, Vec<String>) =
        match periode.as_str() {
            "tag" => match jahr {
                Some(y) => (
                    "date",
                    "WHERE substr(date,1,4) = ?1".to_string(),
                    vec![y.to_string()],
                ),
                None => ("date", "".to_string(), vec![]),
            },
            "monat" => match jahr {
                Some(y) => (
                    "substr(date,1,7)",
                    "WHERE substr(date,1,4) = ?1".to_string(),
                    vec![y.to_string()],
                ),
                None => ("substr(date,1,7)", "".to_string(), vec![]),
            },
            "jahr" => ("substr(date,1,4)", "".to_string(), vec![]),
            "max" => ("'gesamt'", "".to_string(), vec![]),
            _ => return Err(format!("Unknown periode: {}", periode)),
        };
    let sql = format!(
        "SELECT {bucket_expr} AS bucket,
                SUM(erzeugung_kwh)     AS erz,
                SUM(eigenverbrauch_kwh) AS ev,
                SUM(einspeisung_kwh)   AS einsp,
                COUNT(*)               AS tage
         FROM daily_production
         {where_clause}
         GROUP BY bucket
         ORDER BY bucket ASC"
    );
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let p: Vec<&dyn rusqlite::ToSql> = params_vec
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();
    let rows: Vec<Aggregat> = stmt
        .query_map(&p[..], |r| {
            Ok(Aggregat {
                bucket: r.get(0)?,
                erzeugung_kwh: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                eigenverbrauch_kwh: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                einspeisung_kwh: r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                tage: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ── Dashboard ───────────────────────────────────────────────────────────────

#[command]
fn get_dashboard(state: State<DbState>) -> Result<DashboardSnapshot, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let today = Local::now().date_naive();
    let today_iso = today.format("%Y-%m-%d").to_string();
    let woche_start = today - chrono::Duration::days(6);
    let woche_start_iso = woche_start.format("%Y-%m-%d").to_string();
    let monat_start = format!("{}-{:02}-01", today.year(), today.month());
    let jahr_start = format!("{}-01-01", today.year());

    let heute = db
        .query_row(
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz
             FROM daily_production WHERE date = ?1",
            params![today_iso],
            |r| {
                Ok(DailyProduction {
                    date: r.get(0)?,
                    erzeugung_kwh: r.get(1)?,
                    eigenverbrauch_kwh: r.get(2)?,
                    einspeisung_kwh: r.get(3)?,
                    netzbezug_kwh: r.get(4)?,
                    notiz: r.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let sum_range = |from: &str, to: &str| -> Result<f64, String> {
        db.query_row(
            "SELECT COALESCE(SUM(erzeugung_kwh), 0) FROM daily_production
             WHERE date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())
    };
    let woche_kwh = sum_range(&woche_start_iso, &today_iso)?;
    let monat_kwh = sum_range(&monat_start, &today_iso)?;
    let jahr_kwh = sum_range(&jahr_start, &today_iso)?;

    let max_tag = db
        .query_row(
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz
             FROM daily_production
             ORDER BY erzeugung_kwh DESC LIMIT 1",
            [],
            |r| {
                Ok(DailyProduction {
                    date: r.get(0)?,
                    erzeugung_kwh: r.get(1)?,
                    eigenverbrauch_kwh: r.get(2)?,
                    einspeisung_kwh: r.get(3)?,
                    netzbezug_kwh: r.get(4)?,
                    notiz: r.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let einnahmen_jahr: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(netto), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![jahr_start, today_iso],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;

    let stromtarif_perioden = load_stromtarif_perioden(&db).map_err(|e| e.to_string())?;
    let bezugspreis_fallback = get_setting_f64(&db, "strom_bezugspreis", 0.35);
    // Taggenau aufsummieren: ohne Tarif-Verlauf wird der Fallback verwendet.
    let einsparung_jahr: f64 = {
        let mut stmt = db
            .prepare(
                "SELECT date, eigenverbrauch_kwh FROM daily_production
                 WHERE date BETWEEN ?1 AND ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![jahr_start, today_iso], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut sum = 0.0;
        for row in rows.flatten() {
            let (date, kwh) = row;
            let (arbeit, _) = stromtarif_for(&stromtarif_perioden, &date, bezugspreis_fallback);
            sum += kwh * arbeit;
        }
        sum
    };

    let betreiber_perioden = load_betreiber_perioden(&db).map_err(|e| e.to_string())?;
    let betreiber_modus = betreiber_modus_for(&betreiber_perioden, &today_iso);

    Ok(DashboardSnapshot {
        heute,
        woche_kwh,
        monat_kwh,
        jahr_kwh,
        max_tag,
        einnahmen_jahr,
        einsparung_jahr,
        betreiber_modus,
    })
}

// ── EÜR ─────────────────────────────────────────────────────────────────────

/// Aktive AfA-Monate eines Jahres, abhängig von Inbetriebnahme und (optional)
/// Verkauf. Erstjahr ab Inbetriebnahmemonat, Letztjahr bis Verkaufsmonat
/// (Monat des Verkaufs zählt nicht mehr — übliche Praxis).
fn afa_monate(asset: &Asset, jahr: i32) -> i32 {
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0;
    };
    let start = if jahr == ib.year() { ib.month() as i32 } else { 1 };
    let mut ende = 12;
    if jahr < ib.year() {
        return 0;
    }
    if let Some(va) = asset
        .verkauft_am
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
    {
        if jahr > va.year() {
            return 0;
        }
        if jahr == va.year() {
            ende = (va.month() as i32) - 1;
        }
    }
    (ende - start + 1).max(0)
}

fn afa_basis(asset: &Asset) -> f64 {
    asset.anschaffung_netto + asset.anschaffung_ust
}

/// Reguläre AfA im Jahr — kombiniert lineare, GWG-Sofort und Verkauf.
/// Sonder-AfA wird separat in `sonder_afa_for_year` ausgewiesen.
fn afa_for_year(asset: &Asset, jahr: i32) -> f64 {
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0.0;
    };
    if asset.afa_methode == "gwg_sofort" {
        return if jahr == ib.year() { afa_basis(asset) } else { 0.0 };
    }
    // linear
    let nd = asset.nutzungsdauer_jahre.max(1) as f64;
    let yearly = afa_basis(asset) / nd;
    let monate = afa_monate(asset, jahr) as f64;
    if monate <= 0.0 {
        return 0.0;
    }
    let end_year = ib.year() + asset.nutzungsdauer_jahre as i32 - 1;
    if jahr > end_year {
        return 0.0;
    }
    yearly * (monate / 12.0)
}

/// Sonder-AfA §7g Abs. 5 EStG — einmalig im Inbetriebnahmejahr, prozentual auf AK.
fn sonder_afa_for_year(asset: &Asset, jahr: i32) -> f64 {
    if asset.afa_methode == "gwg_sofort" {
        return 0.0;
    }
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0.0;
    };
    if jahr != ib.year() {
        return 0.0;
    }
    let pct = asset.sonderabschreibung_prozent.clamp(0.0, 50.0);
    afa_basis(asset) * pct / 100.0
}

/// Kumulierte Abschreibung (linear + Sonder-AfA) bis einschließlich `jahr`.
fn afa_kumuliert_bis(asset: &Asset, jahr: i32) -> f64 {
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0.0;
    };
    let mut sum = 0.0;
    for y in ib.year()..=jahr {
        sum += afa_for_year(asset, y) + sonder_afa_for_year(asset, y);
    }
    let basis = afa_basis(asset);
    if sum > basis {
        sum = basis;
    }
    sum
}

fn restbuchwert_bis(asset: &Asset, jahr: i32) -> f64 {
    (afa_basis(asset) - afa_kumuliert_bis(asset, jahr)).max(0.0)
}

#[command]
fn get_euer(state: State<DbState>, jahr: i32) -> Result<EuerReport, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let perioden = load_perioden(&db).map_err(|e| e.to_string())?;
    let betreiber_perioden = load_betreiber_perioden(&db).map_err(|e| e.to_string())?;
    let ust_satz = get_setting_f64(&db, "ust_satz_regel", 0.19);
    let ev_preis = get_setting_f64(&db, "eigenverbrauch_preis", 0.20);

    let jahr_str = format!("{:04}", jahr);
    let jahr_start = format!("{}-01-01", jahr_str);
    let jahr_end = format!("{}-12-31", jahr_str);

    let einnahmen_einspeisung_netto: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(netto), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![jahr_start, jahr_end],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;
    let einnahmen_ust_einsp: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![jahr_start, jahr_end],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;

    // Eigenverbrauch (unentgeltliche Wertabgabe) — pro Tag den jeweils gültigen Modus.
    let mut ev_netto = 0.0;
    let mut ev_ust = 0.0;
    {
        let mut stmt = db
            .prepare(
                "SELECT date, eigenverbrauch_kwh FROM daily_production
                 WHERE date BETWEEN ?1 AND ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![jahr_start, jahr_end], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            let (date, kwh) = row;
            let modus = modus_for(&perioden, &date);
            let netto = kwh * ev_preis;
            ev_netto += netto;
            if modus == "regel" {
                ev_ust += netto * ust_satz;
            }
        }
    }

    let (ausgaben_betrieb_netto, ausgaben_betrieb_ust): (f64, f64) = db
        .query_row(
            "SELECT COALESCE(SUM(netto), 0), COALESCE(SUM(ust), 0) FROM expenses
             WHERE date BETWEEN ?1 AND ?2",
            params![jahr_start, jahr_end],
            |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let vorsteuer: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM expenses
             WHERE date BETWEEN ?1 AND ?2 AND vorsteuer_abzugsfaehig = 1",
            params![jahr_start, jahr_end],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;

    let assets = list_assets_internal(&db).map_err(|e| e.to_string())?;
    let ausgaben_afa: f64 = assets.iter().map(|a| afa_for_year(a, jahr)).sum();
    let ausgaben_sonder_afa: f64 = assets.iter().map(|a| sonder_afa_for_year(a, jahr)).sum();

    // Anlagenverkäufe im Jahr: Erlös netto (Einnahme), Restbuchwert zum
    // Beginn des Verkaufsjahres als Aufwand (Abgang). Differenz = Veräußerungs-
    // gewinn/-verlust und fließt automatisch in den Gewinn.
    let mut einnahmen_veraeusserung_netto = 0.0;
    let mut ausgaben_restbuchwert_abgang = 0.0;
    for a in &assets {
        if let Some(va) = a
            .verkauft_am
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        {
            if va.year() == jahr {
                einnahmen_veraeusserung_netto += a.verkaufserloes_netto.unwrap_or(0.0);
                ausgaben_restbuchwert_abgang += restbuchwert_bis(a, jahr);
            }
        }
    }

    let einnahmen_ust = einnahmen_ust_einsp + ev_ust;
    let einnahmen = einnahmen_einspeisung_netto + ev_netto + einnahmen_veraeusserung_netto;
    let ausgaben =
        ausgaben_betrieb_netto + ausgaben_afa + ausgaben_sonder_afa + ausgaben_restbuchwert_abgang;
    let gewinn = einnahmen - ausgaben;

    let betreiber_modus = betreiber_modus_for(&betreiber_perioden, &jahr_end);
    let (est_pflichtig, est_befreiungsgrund) = if betreiber_modus == "privat" {
        (
            false,
            Some(
                "Privatbetrieb — Einkommensteuer-Befreiung nach §3 Nr. 72 EStG \
                 (PV-Anlage bis 30 kWp / 15 kWp je Wohneinheit). Werte werden \
                 informativ ausgewiesen, fließen aber nicht in die ESt-Erklärung."
                    .to_string(),
            ),
        )
    } else {
        (true, None)
    };

    Ok(EuerReport {
        jahr,
        einnahmen_einspeisung_netto,
        einnahmen_eigenverbrauch_netto: ev_netto,
        einnahmen_veraeusserung_netto,
        einnahmen_ust,
        ausgaben_betrieb_netto,
        ausgaben_betrieb_ust,
        ausgaben_afa,
        ausgaben_sonder_afa,
        ausgaben_restbuchwert_abgang,
        vorsteuer,
        gewinn_vor_steuern: gewinn,
        betreiber_modus,
        est_pflichtig,
        est_befreiungsgrund,
    })
}

fn list_assets_internal(conn: &Connection) -> Result<Vec<Asset>, rusqlite::Error> {
    let sql = format!("SELECT {ASSET_COLS} FROM assets ORDER BY inbetriebnahme ASC, id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], map_asset)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ── UStVA ───────────────────────────────────────────────────────────────────

#[command]
fn get_ustva(
    state: State<DbState>,
    jahr: i32,
    monat: Option<i32>,
) -> Result<UstvaReport, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let perioden = load_perioden(&db).map_err(|e| e.to_string())?;
    let ust_satz = get_setting_f64(&db, "ust_satz_regel", 0.19);
    let ev_preis = get_setting_f64(&db, "eigenverbrauch_preis", 0.20);

    let (from, to) = match monat {
        Some(m) => {
            let start = NaiveDate::from_ymd_opt(jahr, m as u32, 1)
                .ok_or_else(|| format!("Ungültiger Monat: {}/{}", m, jahr))?;
            let next = if m == 12 {
                NaiveDate::from_ymd_opt(jahr + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(jahr, (m + 1) as u32, 1).unwrap()
            };
            let end = next - chrono::Duration::days(1);
            (
                start.format("%Y-%m-%d").to_string(),
                end.format("%Y-%m-%d").to_string(),
            )
        }
        None => (format!("{:04}-01-01", jahr), format!("{:04}-12-31", jahr)),
    };

    // Aktiver Modus = Modus am Periodenende (Stichtag).
    let modus = modus_for(&perioden, &to);

    let ust_einnahmen_payouts: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;
    let ust_einnahmen_verkauf: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(verkaufserloes_ust), 0) FROM assets
             WHERE verkauft_am IS NOT NULL AND verkauft_am BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;
    let ust_einnahmen = ust_einnahmen_payouts + ust_einnahmen_verkauf;

    let ev_kwh: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(eigenverbrauch_kwh), 0) FROM daily_production
             WHERE date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;
    let ust_eigenverbrauch = if modus == "regel" {
        ev_kwh * ev_preis * ust_satz
    } else {
        0.0
    };

    let vorsteuer: f64 = if modus == "regel" {
        db.query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM expenses
             WHERE date BETWEEN ?1 AND ?2 AND vorsteuer_abzugsfaehig = 1",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        0.0
    };

    // Kleinunternehmer erhebt keine USt und führt keine ab.
    let (ust_einnahmen, ust_eigenverbrauch, vorsteuer) = if modus == "kleinunternehmer" {
        (0.0, 0.0, 0.0)
    } else {
        (ust_einnahmen, ust_eigenverbrauch, vorsteuer)
    };

    let zahllast = ust_einnahmen + ust_eigenverbrauch - vorsteuer;

    Ok(UstvaReport {
        jahr,
        monat,
        modus,
        ust_einnahmen,
        ust_eigenverbrauch,
        vorsteuer,
        zahllast,
    })
}

// ── Erwartete Einspeisevergütung ────────────────────────────────────────────

#[command]
fn get_expected_einspeisung(
    state: State<DbState>,
    jahr: i32,
    monat: Option<i32>,
) -> Result<ExpectedEinspeisung, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let verguetung_perioden = load_verguetung_perioden(&db).map_err(|e| e.to_string())?;

    let (from, to) = match monat {
        Some(m) => {
            let start = NaiveDate::from_ymd_opt(jahr, m as u32, 1)
                .ok_or_else(|| format!("Ungültiger Monat: {}/{}", m, jahr))?;
            let next = if m == 12 {
                NaiveDate::from_ymd_opt(jahr + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(jahr, (m + 1) as u32, 1).unwrap()
            };
            let end = next - chrono::Duration::days(1);
            (
                start.format("%Y-%m-%d").to_string(),
                end.format("%Y-%m-%d").to_string(),
            )
        }
        None => (format!("{:04}-01-01", jahr), format!("{:04}-12-31", jahr)),
    };

    let mut stmt = db
        .prepare(
            "SELECT date, einspeisung_kwh FROM daily_production
             WHERE date BETWEEN ?1 AND ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![from, to], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut kwh_sum = 0.0;
    let mut erwartet_netto = 0.0;
    let mut tage_ohne_satz: i64 = 0;
    for row in rows.flatten() {
        let (date, kwh) = row;
        kwh_sum += kwh;
        match verguetung_for(&verguetung_perioden, &date) {
            Some((satz_eur, _modell)) => erwartet_netto += kwh * satz_eur,
            None => {
                if kwh > 0.0 {
                    tage_ohne_satz += 1;
                }
            }
        }
    }

    Ok(ExpectedEinspeisung {
        jahr,
        monat,
        kwh: kwh_sum,
        erwartet_netto,
        tage_ohne_satz,
    })
}

// ── CSV / JSON Export ───────────────────────────────────────────────────────

fn csv_escape(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn fmt_de(v: f64) -> String {
    format!("{:.2}", v).replace('.', ",")
}

#[command]
fn export_buchungen_csv(state: State<DbState>, path: String, jahr: i32) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let jahr_start = format!("{:04}-01-01", jahr);
    let jahr_end = format!("{:04}-12-31", jahr);

    let mut out = String::new();
    out.push_str(
        "Datum;Typ;Beleg;Beschreibung;Kategorie;Netto;USt;Brutto;Vorsteuer\r\n",
    );
    let mut rows: i64 = 0;

    // Einnahmen aus Auszahlungen
    let mut stmt = db
        .prepare(
            "SELECT id, buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto, notiz
             FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2
             ORDER BY buchung_date ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let payout_rows = stmt
        .query_map(params![jahr_start, jahr_end], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in payout_rows.flatten() {
        let (id, datum, von, bis, netto, ust, brutto, notiz) = row;
        let beschreibung = format!(
            "Einspeisevergütung {} – {}{}",
            von,
            bis,
            notiz.map(|n| format!(" ({})", n)).unwrap_or_default()
        );
        out.push_str(&format!(
            "{};Einnahme;Auszahlung-{};{};{};{};{};{};\r\n",
            datum,
            id,
            csv_escape(&beschreibung),
            csv_escape("Einspeisung"),
            fmt_de(netto),
            fmt_de(ust),
            fmt_de(brutto),
        ));
        rows += 1;
    }

    // Ausgaben
    let mut stmt = db
        .prepare(
            "SELECT id, date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig
             FROM expenses
             WHERE date BETWEEN ?1 AND ?2
             ORDER BY date ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let exp_rows = stmt
        .query_map(params![jahr_start, jahr_end], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, i64>(7)? != 0,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in exp_rows.flatten() {
        let (id, datum, kat, beschr, netto, ust, brutto, vsa) = row;
        let vorsteuer = if vsa { fmt_de(ust) } else { String::new() };
        out.push_str(&format!(
            "{};Ausgabe;Ausgabe-{};{};{};{};{};{};{}\r\n",
            datum,
            id,
            csv_escape(&beschr),
            csv_escape(&kat),
            fmt_de(netto),
            fmt_de(ust),
            fmt_de(brutto),
            vorsteuer,
        ));
        rows += 1;
    }

    // UTF-8 BOM voranstellen, damit Excel die Sonderzeichen erkennt.
    let mut bytes = Vec::with_capacity(out.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(out.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("Datei schreiben fehlgeschlagen: {}", e))?;
    Ok(rows)
}

#[command]
fn export_anlagen_csv(state: State<DbState>, path: String) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let assets = list_assets_internal(&db).map_err(|e| e.to_string())?;
    let mut out = String::new();
    out.push_str(
        "Name;Inbetriebnahme;Anschaffung Netto;Anschaffung USt;Anschaffung Brutto;\
         Nutzungsdauer (Jahre);AfA pro Jahr;Notiz\r\n",
    );
    for a in &assets {
        let basis = a.anschaffung_netto + a.anschaffung_ust;
        let afa = basis / (a.nutzungsdauer_jahre.max(1) as f64);
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{}\r\n",
            csv_escape(&a.name),
            a.inbetriebnahme,
            fmt_de(a.anschaffung_netto),
            fmt_de(a.anschaffung_ust),
            fmt_de(basis),
            a.nutzungsdauer_jahre,
            fmt_de(afa),
            csv_escape(a.notiz.as_deref().unwrap_or("")),
        ));
    }
    let mut bytes = Vec::with_capacity(out.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(out.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("Datei schreiben fehlgeschlagen: {}", e))?;
    Ok(assets.len() as i64)
}

#[derive(Serialize, Deserialize)]
struct Backup {
    version: u32,
    exported_at: String,
    daily_production: Vec<DailyProduction>,
    payouts: Vec<Payout>,
    expenses: Vec<Expense>,
    assets: Vec<Asset>,
    ust_perioden: Vec<UstPeriode>,
    betreiber_perioden: Vec<BetreiberPeriode>,
    verguetung_perioden: Vec<VerguetungPeriode>,
    #[serde(default)]
    stromtarif_perioden: Vec<StromtarifPeriode>,
    settings: SettingsKV,
}

#[derive(Serialize, Deserialize, Default)]
struct SettingsKV {
    ust_satz_regel: f64,
    eigenverbrauch_preis: f64,
    strom_bezugspreis: f64,
    anker_api_url: Option<String>,
    anker_api_token: Option<String>,
}

#[derive(Serialize)]
pub struct BackupSummary {
    pub daily: i64,
    pub payouts: i64,
    pub expenses: i64,
    pub assets: i64,
}

fn collect_backup(conn: &Connection) -> Result<Backup, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz
         FROM daily_production ORDER BY date ASC",
    )?;
    let daily: Vec<DailyProduction> = stmt
        .query_map([], |r| {
            Ok(DailyProduction {
                date: r.get(0)?,
                erzeugung_kwh: r.get(1)?,
                eigenverbrauch_kwh: r.get(2)?,
                einspeisung_kwh: r.get(3)?,
                netzbezug_kwh: r.get(4)?,
                notiz: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT id, buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto, kwh, notiz
         FROM payouts ORDER BY id ASC",
    )?;
    let payouts: Vec<Payout> = stmt
        .query_map([], |r| {
            Ok(Payout {
                id: r.get(0)?,
                buchung_date: r.get(1)?,
                zeitraum_von: r.get(2)?,
                zeitraum_bis: r.get(3)?,
                netto: r.get(4)?,
                ust: r.get(5)?,
                brutto: r.get(6)?,
                kwh: r.get(7)?,
                notiz: r.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT id, date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig
         FROM expenses ORDER BY id ASC",
    )?;
    let expenses: Vec<Expense> = stmt
        .query_map([], |r| {
            Ok(Expense {
                id: r.get(0)?,
                date: r.get(1)?,
                kategorie: r.get(2)?,
                beschreibung: r.get(3)?,
                netto: r.get(4)?,
                ust: r.get(5)?,
                brutto: r.get(6)?,
                vorsteuer_abzugsfaehig: r.get::<_, i64>(7)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let assets = list_assets_internal(conn)?;
    let ust_perioden = load_perioden(conn)?;
    let betreiber_perioden = load_betreiber_perioden(conn)?;
    let verguetung_perioden = load_verguetung_perioden(conn)?;
    let stromtarif_perioden = load_stromtarif_perioden(conn)?;

    let settings = SettingsKV {
        ust_satz_regel: get_setting_f64(conn, "ust_satz_regel", 0.19),
        eigenverbrauch_preis: get_setting_f64(conn, "eigenverbrauch_preis", 0.20),
        strom_bezugspreis: get_setting_f64(conn, "strom_bezugspreis", 0.35),
        anker_api_url: get_setting(conn, "anker_api_url")?.filter(|s| !s.is_empty()),
        anker_api_token: get_setting(conn, "anker_api_token")?.filter(|s| !s.is_empty()),
    };

    Ok(Backup {
        version: 1,
        exported_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        daily_production: daily,
        payouts,
        expenses,
        assets,
        ust_perioden,
        betreiber_perioden,
        verguetung_perioden,
        stromtarif_perioden,
        settings,
    })
}

#[command]
fn export_backup(state: State<DbState>, path: String) -> Result<BackupSummary, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let backup = collect_backup(&db).map_err(|e| e.to_string())?;
    let summary = BackupSummary {
        daily: backup.daily_production.len() as i64,
        payouts: backup.payouts.len() as i64,
        expenses: backup.expenses.len() as i64,
        assets: backup.assets.len() as i64,
    };
    let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Datei schreiben fehlgeschlagen: {}", e))?;
    Ok(summary)
}

#[command]
fn import_backup(state: State<DbState>, path: String) -> Result<BackupSummary, String> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Datei lesen fehlgeschlagen: {}", e))?;
    let backup: Backup = serde_json::from_str(&json)
        .map_err(|e| format!("JSON-Format ungültig: {}", e))?;
    if backup.version != 1 {
        return Err(format!(
            "Unbekannte Backup-Version {} (erwartet 1).",
            backup.version
        ));
    }

    let mut db = state.0.lock().map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM daily_production", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM payouts", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM expenses", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM assets", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM ust_perioden", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM betreiber_perioden", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM verguetung_perioden", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM stromtarif_perioden", [])
        .map_err(|e| e.to_string())?;

    for d in &backup.daily_production {
        tx.execute(
            "INSERT INTO daily_production
             (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh, notiz)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                d.date,
                d.erzeugung_kwh,
                d.eigenverbrauch_kwh,
                d.einspeisung_kwh,
                d.netzbezug_kwh,
                d.notiz
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    for p in &backup.payouts {
        tx.execute(
            "INSERT INTO payouts
             (buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto, kwh, notiz)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                p.buchung_date,
                p.zeitraum_von,
                p.zeitraum_bis,
                p.netto,
                p.ust,
                p.brutto,
                p.kwh,
                p.notiz
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    for e in &backup.expenses {
        tx.execute(
            "INSERT INTO expenses
             (date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                e.date,
                e.kategorie,
                e.beschreibung,
                e.netto,
                e.ust,
                e.brutto,
                if e.vorsteuer_abzugsfaehig { 1i64 } else { 0i64 }
            ],
        )
        .map_err(|err| err.to_string())?;
    }
    for a in &backup.assets {
        tx.execute(
            "INSERT INTO assets
             (name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
              nutzungsdauer_jahre, afa_methode, sonderabschreibung_prozent,
              verkauft_am, verkaufserloes_netto, verkaufserloes_ust, notiz)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                a.name,
                a.inbetriebnahme,
                a.anschaffung_netto,
                a.anschaffung_ust,
                a.nutzungsdauer_jahre,
                a.afa_methode,
                a.sonderabschreibung_prozent,
                a.verkauft_am,
                a.verkaufserloes_netto,
                a.verkaufserloes_ust,
                a.notiz
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    for p in &backup.ust_perioden {
        tx.execute(
            "INSERT INTO ust_perioden (effective_from, modus) VALUES (?1, ?2)",
            params![p.effective_from, p.modus],
        )
        .map_err(|e| e.to_string())?;
    }
    for p in &backup.betreiber_perioden {
        tx.execute(
            "INSERT INTO betreiber_perioden (effective_from, modus) VALUES (?1, ?2)",
            params![p.effective_from, p.modus],
        )
        .map_err(|e| e.to_string())?;
    }
    for p in &backup.verguetung_perioden {
        tx.execute(
            "INSERT INTO verguetung_perioden (effective_from, modell, satz_ct_per_kwh)
             VALUES (?1, ?2, ?3)",
            params![p.effective_from, p.modell, p.satz_ct_per_kwh],
        )
        .map_err(|e| e.to_string())?;
    }
    for p in &backup.stromtarif_perioden {
        tx.execute(
            "INSERT INTO stromtarif_perioden
             (effective_from, arbeitspreis_eur_per_kwh, grundgebuehr_eur_per_monat)
             VALUES (?1, ?2, ?3)",
            params![
                p.effective_from,
                p.arbeitspreis_eur_per_kwh,
                p.grundgebuehr_eur_per_monat
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    // Re-seed Defaults bei leeren Pflicht-Verläufen.
    let cnt: i64 = tx
        .query_row("SELECT COUNT(*) FROM ust_perioden", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if cnt == 0 {
        tx.execute(
            "INSERT INTO ust_perioden (effective_from, modus) VALUES ('2000-01-01', 'regel')",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    let cnt: i64 = tx
        .query_row("SELECT COUNT(*) FROM betreiber_perioden", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if cnt == 0 {
        tx.execute(
            "INSERT INTO betreiber_perioden (effective_from, modus)
             VALUES ('2000-01-01', 'gewerblich')",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    // Settings (Key/Value)
    let s = &backup.settings;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('ust_satz_regel', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.ust_satz_regel.to_string()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('eigenverbrauch_preis', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.eigenverbrauch_preis.to_string()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('strom_bezugspreis', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.strom_bezugspreis.to_string()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('anker_api_url', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.anker_api_url.clone().unwrap_or_default()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('anker_api_token', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.anker_api_token.clone().unwrap_or_default()],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(BackupSummary {
        daily: backup.daily_production.len() as i64,
        payouts: backup.payouts.len() as i64,
        expenses: backup.expenses.len() as i64,
        assets: backup.assets.len() as i64,
    })
}

// ── Vendor-API Import (Stub) ────────────────────────────────────────────────

#[derive(Serialize)]
struct ImportResult {
    imported: i64,
}

#[command]
fn import_from_vendor(
    state: State<DbState>,
    _von: String,
    _bis: String,
) -> Result<ImportResult, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let url = get_setting(&db, "anker_api_url")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    if url.is_none() {
        return Err(
            "Keine Hersteller-API konfiguriert. \
             Hinterlege URL und Token unter Einstellungen → Anker / Vendor-API."
                .into(),
        );
    }
    // TODO: HTTP-Aufruf gegen URL + Token, parse JSON, INSERT/UPDATE daily_production.
    // Implementierung folgt sobald die konkrete API (Anker SOLIX / Fronius / SMA …)
    // feststeht.
    Err("Hersteller-API-Import ist noch nicht implementiert. \
         Bitte Tageswerte vorerst manuell erfassen."
        .into())
}

// ── Entry point ─────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = open_db().expect("Failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            list_daily_range,
            get_daily,
            upsert_daily,
            delete_daily,
            list_payouts,
            upsert_payout,
            delete_payout,
            list_expenses,
            upsert_expense,
            delete_expense,
            list_assets,
            upsert_asset,
            delete_asset,
            get_settings,
            set_settings,
            aggregate_production,
            get_dashboard,
            get_euer,
            get_ustva,
            get_expected_einspeisung,
            export_buchungen_csv,
            export_anlagen_csv,
            export_backup,
            import_backup,
            import_from_vendor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
