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
    #[serde(default)]
    pub notiz: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UstPeriode {
    pub id: i64,
    pub effective_from: String,
    /// "regel" | "kleinunternehmer" | "nullsteuer"
    pub modus: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub ust_perioden: Vec<UstPeriode>,
    pub ust_satz_regel: f64,
    pub eigenverbrauch_preis: f64,
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
    pub einnahmen_ust: f64,
    pub ausgaben_betrieb_netto: f64,
    pub ausgaben_betrieb_ust: f64,
    pub ausgaben_afa: f64,
    pub vorsteuer: f64,
    pub gewinn_vor_steuern: f64,
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

         CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
         );",
    )?;
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
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM ust_perioden", [], |r| r.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO ust_perioden (effective_from, modus) VALUES ('2000-01-01', 'regel')",
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
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, notiz
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
                notiz: r.get(4)?,
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
        "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, notiz
         FROM daily_production WHERE date = ?1",
        params![date],
        |r| {
            Ok(DailyProduction {
                date: r.get(0)?,
                erzeugung_kwh: r.get(1)?,
                eigenverbrauch_kwh: r.get(2)?,
                einspeisung_kwh: r.get(3)?,
                notiz: r.get(4)?,
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
         (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, notiz)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(date) DO UPDATE SET
            erzeugung_kwh = excluded.erzeugung_kwh,
            eigenverbrauch_kwh = excluded.eigenverbrauch_kwh,
            einspeisung_kwh = excluded.einspeisung_kwh,
            notiz = excluded.notiz",
        params![
            entry.date,
            entry.erzeugung_kwh,
            entry.eigenverbrauch_kwh,
            entry.einspeisung_kwh,
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

#[command]
fn list_assets(state: State<DbState>) -> Result<Vec<Asset>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT id, name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
                    nutzungsdauer_jahre, notiz
             FROM assets ORDER BY inbetriebnahme ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<Asset> = stmt
        .query_map([], |r| {
            Ok(Asset {
                id: r.get(0)?,
                name: r.get(1)?,
                inbetriebnahme: r.get(2)?,
                anschaffung_netto: r.get(3)?,
                anschaffung_ust: r.get(4)?,
                nutzungsdauer_jahre: r.get(5)?,
                notiz: r.get(6)?,
            })
        })
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
                anschaffung_ust = ?4, nutzungsdauer_jahre = ?5, notiz = ?6
             WHERE id = ?7",
            params![
                asset.name,
                asset.inbetriebnahme,
                asset.anschaffung_netto,
                asset.anschaffung_ust,
                asset.nutzungsdauer_jahre,
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
              nutzungsdauer_jahre, notiz)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                asset.name,
                asset.inbetriebnahme,
                asset.anschaffung_netto,
                asset.anschaffung_ust,
                asset.nutzungsdauer_jahre,
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
    let perioden = load_perioden(&db).map_err(|e| e.to_string())?;
    let url = get_setting(&db, "anker_api_url")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    let token = get_setting(&db, "anker_api_token")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    Ok(Settings {
        ust_perioden: perioden,
        ust_satz_regel: get_setting_f64(&db, "ust_satz_regel", 0.19),
        eigenverbrauch_preis: get_setting_f64(&db, "eigenverbrauch_preis", 0.20),
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
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, notiz
             FROM daily_production WHERE date = ?1",
            params![today_iso],
            |r| {
                Ok(DailyProduction {
                    date: r.get(0)?,
                    erzeugung_kwh: r.get(1)?,
                    eigenverbrauch_kwh: r.get(2)?,
                    einspeisung_kwh: r.get(3)?,
                    notiz: r.get(4)?,
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
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, notiz
             FROM daily_production
             ORDER BY erzeugung_kwh DESC LIMIT 1",
            [],
            |r| {
                Ok(DailyProduction {
                    date: r.get(0)?,
                    erzeugung_kwh: r.get(1)?,
                    eigenverbrauch_kwh: r.get(2)?,
                    einspeisung_kwh: r.get(3)?,
                    notiz: r.get(4)?,
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

    Ok(DashboardSnapshot {
        heute,
        woche_kwh,
        monat_kwh,
        jahr_kwh,
        max_tag,
        einnahmen_jahr,
    })
}

// ── EÜR ─────────────────────────────────────────────────────────────────────

fn afa_for_year(asset: &Asset, jahr: i32) -> f64 {
    let nd = asset.nutzungsdauer_jahre.max(1) as f64;
    let basis = asset.anschaffung_netto + asset.anschaffung_ust;
    let yearly = basis / nd;
    let ib = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok();
    let Some(ib) = ib else {
        return 0.0;
    };
    let start_year = ib.year();
    let end_year = start_year + asset.nutzungsdauer_jahre as i32 - 1;
    if jahr < start_year || jahr > end_year {
        return 0.0;
    }
    if jahr == start_year {
        // Pro-rata for the start year (Monate ab Inbetriebnahme / 12).
        let monate = (13 - ib.month() as i32) as f64;
        return yearly * (monate / 12.0);
    }
    yearly
}

#[command]
fn get_euer(state: State<DbState>, jahr: i32) -> Result<EuerReport, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let perioden = load_perioden(&db).map_err(|e| e.to_string())?;
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

    let einnahmen_ust = einnahmen_ust_einsp + ev_ust;
    let einnahmen = einnahmen_einspeisung_netto + ev_netto;
    let ausgaben = ausgaben_betrieb_netto + ausgaben_afa;
    let gewinn = einnahmen - ausgaben;

    Ok(EuerReport {
        jahr,
        einnahmen_einspeisung_netto,
        einnahmen_eigenverbrauch_netto: ev_netto,
        einnahmen_ust,
        ausgaben_betrieb_netto,
        ausgaben_betrieb_ust,
        ausgaben_afa,
        vorsteuer,
        gewinn_vor_steuern: gewinn,
    })
}

fn list_assets_internal(conn: &Connection) -> Result<Vec<Asset>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
                nutzungsdauer_jahre, notiz
         FROM assets",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Asset {
                id: r.get(0)?,
                name: r.get(1)?,
                inbetriebnahme: r.get(2)?,
                anschaffung_netto: r.get(3)?,
                anschaffung_ust: r.get(4)?,
                nutzungsdauer_jahre: r.get(5)?,
                notiz: r.get(6)?,
            })
        })?
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

    let ust_einnahmen: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;

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
            import_from_vendor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
