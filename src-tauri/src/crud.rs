//! CRUD-Commands für die Hauptentitäten und Settings.
//!
//! Pro Entität ein Trio aus `list_*` / `upsert_*` / `delete_*`. Settings sind
//! ein Sonderfall: `get_settings` / `set_settings` arbeiten als Bulk-Endpoint
//! über Key/Value-Tabelle + die vier Verlaufstabellen.

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{command, State};

use crate::db::{get_cents, get_cents_opt, get_setting, get_setting_f64, set_setting, DbState};
use crate::types::{Asset, DailyProduction, Expense, Payout, Settings};
use crate::verlauf::{
    load_betreiber_perioden, load_perioden, load_stromtarif_perioden, load_verguetung_perioden,
};

// ── Tageserfassung ──────────────────────────────────────────────────────────

#[command]
pub fn list_daily_range(
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
pub fn get_daily(
    state: State<DbState>,
    date: String,
) -> Result<Option<DailyProduction>, String> {
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
pub fn upsert_daily(state: State<DbState>, entry: DailyProduction) -> Result<(), String> {
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
pub fn delete_daily(state: State<DbState>, date: String) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM daily_production WHERE date = ?1", params![date])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Payouts ─────────────────────────────────────────────────────────────────

#[command]
pub fn list_payouts(state: State<DbState>) -> Result<Vec<Payout>, String> {
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
                netto: get_cents(r, 4)?,
                ust: get_cents(r, 5)?,
                brutto: get_cents(r, 6)?,
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
pub fn upsert_payout(state: State<DbState>, payout: Payout) -> Result<i64, String> {
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
pub fn delete_payout(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM payouts WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Expenses ────────────────────────────────────────────────────────────────

#[command]
pub fn list_expenses(state: State<DbState>) -> Result<Vec<Expense>, String> {
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
                netto: get_cents(r, 4)?,
                ust: get_cents(r, 5)?,
                brutto: get_cents(r, 6)?,
                vorsteuer_abzugsfaehig: r.get::<_, i64>(7)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[command]
pub fn upsert_expense(state: State<DbState>, expense: Expense) -> Result<i64, String> {
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
pub fn delete_expense(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM expenses WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Assets / AfA ────────────────────────────────────────────────────────────

pub(crate) const ASSET_COLS: &str = "id, name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
    nutzungsdauer_jahre, afa_methode, sonderabschreibung_prozent,
    verkauft_am, verkaufserloes_netto, verkaufserloes_ust, notiz";

pub(crate) fn map_asset(r: &rusqlite::Row) -> rusqlite::Result<Asset> {
    Ok(Asset {
        id: r.get(0)?,
        name: r.get(1)?,
        inbetriebnahme: r.get(2)?,
        anschaffung_netto: get_cents(r, 3)?,
        anschaffung_ust: get_cents(r, 4)?,
        nutzungsdauer_jahre: r.get(5)?,
        afa_methode: r.get(6)?,
        sonderabschreibung_prozent: r.get(7)?,
        verkauft_am: r.get(8)?,
        verkaufserloes_netto: get_cents_opt(r, 9)?,
        verkaufserloes_ust: get_cents_opt(r, 10)?,
        notiz: r.get(11)?,
    })
}

pub(crate) fn list_assets_internal(conn: &Connection) -> Result<Vec<Asset>, rusqlite::Error> {
    let sql = format!("SELECT {ASSET_COLS} FROM assets ORDER BY inbetriebnahme ASC, id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], map_asset)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[command]
pub fn list_assets(state: State<DbState>) -> Result<Vec<Asset>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    list_assets_internal(&db).map_err(|e| e.to_string())
}

#[command]
pub fn upsert_asset(state: State<DbState>, asset: Asset) -> Result<i64, String> {
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
pub fn delete_asset(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM assets WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Settings ────────────────────────────────────────────────────────────────

#[command]
pub fn get_settings(state: State<DbState>) -> Result<Settings, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let ust_perioden = load_perioden(&db).map_err(|e| e.to_string())?;
    let betreiber_perioden = load_betreiber_perioden(&db).map_err(|e| e.to_string())?;
    let verguetung_perioden = load_verguetung_perioden(&db).map_err(|e| e.to_string())?;
    let stromtarif_perioden = load_stromtarif_perioden(&db).map_err(|e| e.to_string())?;
    let email = get_setting(&db, "anker_email")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    let password = get_setting(&db, "anker_password")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty());
    let country = get_setting(&db, "anker_country")
        .map_err(|e| e.to_string())?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "DE".to_string());
    Ok(Settings {
        ust_perioden,
        betreiber_perioden,
        verguetung_perioden,
        stromtarif_perioden,
        ust_satz_regel: get_setting_f64(&db, "ust_satz_regel", 0.19),
        eigenverbrauch_preis: get_setting_f64(&db, "eigenverbrauch_preis", 0.20),
        strom_bezugspreis: get_setting_f64(&db, "strom_bezugspreis", 0.35),
        anker_email: email,
        anker_password: password,
        anker_country: country,
    })
}

#[command]
pub fn set_settings(state: State<DbState>, settings: Settings) -> Result<(), String> {
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
        "anker_email",
        settings.anker_email.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;
    set_setting(
        &db,
        "anker_password",
        settings.anker_password.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;
    let country = if settings.anker_country.trim().is_empty() {
        "DE"
    } else {
        settings.anker_country.as_str()
    };
    set_setting(&db, "anker_country", country).map_err(|e| e.to_string())?;

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
