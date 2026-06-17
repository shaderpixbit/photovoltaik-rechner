//! Verbindung, Schema-Anlage, additive Migrationen, Seeds und
//! Settings-Key/Value-Helper. Wird beim App-Start einmal aufgerufen.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct DbState(pub Mutex<Connection>);

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
    migrate_money_to_cents(&conn)?;
    Ok(conn)
}

/// One-shot Konvertierung aller Geldspalten von €-`REAL` zu Cents-`INTEGER`.
/// Läuft idempotent: speichert ein Marker-Setting `migration_money_cents_v1`.
/// SQLite hat dynamische Typen — die Spalten-Deklarationen bleiben unverändert,
/// nur die gespeicherten Werte ändern sich.
pub(crate) fn migrate_money_to_cents(conn: &Connection) -> Result<(), rusqlite::Error> {
    let applied: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'migration_money_cents_v1'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if applied.is_some() {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN;
         UPDATE payouts SET
            netto  = CAST(ROUND(netto  * 100) AS INTEGER),
            ust    = CAST(ROUND(ust    * 100) AS INTEGER),
            brutto = CAST(ROUND(brutto * 100) AS INTEGER);
         UPDATE expenses SET
            netto  = CAST(ROUND(netto  * 100) AS INTEGER),
            ust    = CAST(ROUND(ust    * 100) AS INTEGER),
            brutto = CAST(ROUND(brutto * 100) AS INTEGER);
         UPDATE assets SET
            anschaffung_netto    = CAST(ROUND(anschaffung_netto    * 100) AS INTEGER),
            anschaffung_ust      = CAST(ROUND(anschaffung_ust      * 100) AS INTEGER),
            verkaufserloes_netto = CASE WHEN verkaufserloes_netto IS NULL THEN NULL
                                        ELSE CAST(ROUND(verkaufserloes_netto * 100) AS INTEGER) END,
            verkaufserloes_ust   = CASE WHEN verkaufserloes_ust IS NULL THEN NULL
                                        ELSE CAST(ROUND(verkaufserloes_ust * 100) AS INTEGER) END;
         UPDATE stromtarif_perioden SET
            grundgebuehr_eur_per_monat = CAST(ROUND(grundgebuehr_eur_per_monat * 100) AS INTEGER);
         INSERT OR REPLACE INTO settings (key, value)
            VALUES ('migration_money_cents_v1', 'applied');
         COMMIT;",
    )?;
    Ok(())
}

pub(crate) fn create_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
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
            netto           INTEGER NOT NULL DEFAULT 0,
            ust             INTEGER NOT NULL DEFAULT 0,
            brutto          INTEGER NOT NULL DEFAULT 0,
            kwh             REAL,
            notiz           TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_payouts_year ON payouts(substr(buchung_date,1,4));

         CREATE TABLE IF NOT EXISTS expenses (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            date            TEXT NOT NULL,
            kategorie       TEXT NOT NULL,
            beschreibung    TEXT NOT NULL DEFAULT '',
            netto           INTEGER NOT NULL DEFAULT 0,
            ust             INTEGER NOT NULL DEFAULT 0,
            brutto          INTEGER NOT NULL DEFAULT 0,
            vorsteuer_abzugsfaehig INTEGER NOT NULL DEFAULT 1
         );
         CREATE INDEX IF NOT EXISTS idx_expenses_year ON expenses(substr(date,1,4));

         CREATE TABLE IF NOT EXISTS assets (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            name               TEXT NOT NULL,
            inbetriebnahme     TEXT NOT NULL,
            anschaffung_netto  INTEGER NOT NULL DEFAULT 0,
            anschaffung_ust    INTEGER NOT NULL DEFAULT 0,
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
            grundgebuehr_eur_per_monat  INTEGER NOT NULL DEFAULT 0
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

pub(crate) fn seed_defaults(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('ust_satz_regel', '0.19')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('eigenverbrauch_preis', '0.20')",
        [],
    )?;
    // Vendor-Wahl: Migration fuer Bestandsnutzer — wenn anker_email schon
    // gesetzt ist, default auf "anker", sonst "none".
    let prior_email: String = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'anker_email'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let default_vendor = if prior_email.is_empty() { "none" } else { "anker" };
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('vendor', ?1)",
        params![default_vendor],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('anker_email', '')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('anker_password', '')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('anker_country', 'DE')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('solaredge_api_key', '')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('solaredge_site_id', '')",
        [],
    )?;
    // Alte URL/Token-Keys aus dem Stub aufraeumen, falls noch vorhanden.
    conn.execute(
        "DELETE FROM settings WHERE key IN ('anker_api_url', 'anker_api_token')",
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

/// Liest eine Cents-Spalte robust — egal ob die Zelle `INTEGER` oder
/// `REAL` ist (Altbestand vor der Cents-Migration kann noch REAL sein).
pub(crate) fn get_cents(r: &rusqlite::Row, idx: usize) -> rusqlite::Result<i64> {
    let v: f64 = r.get(idx)?;
    Ok(v.round() as i64)
}

/// Wie `get_cents`, aber für nullable Spalten.
pub(crate) fn get_cents_opt(r: &rusqlite::Row, idx: usize) -> rusqlite::Result<Option<i64>> {
    let v: Option<f64> = r.get(idx)?;
    Ok(v.map(|f| f.round() as i64))
}

pub(crate) fn get_setting(
    conn: &Connection,
    key: &str,
) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
}

pub(crate) fn get_setting_f64(conn: &Connection, key: &str, default: f64) -> f64 {
    get_setting(conn, key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}

pub(crate) fn set_setting(
    conn: &Connection,
    key: &str,
    value: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
