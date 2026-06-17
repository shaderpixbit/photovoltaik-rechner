//! Lokale Exporte: CSV-Buchungen, CSV-Anlagenverzeichnis, JSON-Backup/Restore,
//! Vendor-API-Import-Stub. Pfade kommen aus dem JS-Save/Open-Dialog —
//! Rust schreibt direkt via `std::fs`.

use chrono::Local;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Emitter, State};

use crate::crud::list_assets_internal;
use crate::db::{get_cents, get_setting, get_setting_f64, DbState};
use crate::types::{
    Asset, BackupSummary, BetreiberPeriode, DailyProduction, Expense, Payout,
    StromtarifPeriode, UstPeriode, VerguetungPeriode,
};
use crate::verlauf::{
    load_betreiber_perioden, load_perioden, load_stromtarif_perioden, load_verguetung_perioden,
};

fn csv_escape(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Formatiert Cents als deutsche Währungs-Darstellung (Komma als Dezimaltrenner).
fn fmt_cents(cents: i64) -> String {
    let eur = cents as f64 / 100.0;
    format!("{:.2}", eur).replace('.', ",")
}

fn fmt_f64(v: f64) -> String {
    format!("{:.2}", v).replace('.', ",")
}

#[command]
pub fn export_buchungen_csv(
    state: State<DbState>,
    path: String,
    jahr: i32,
) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let jahr_start = format!("{:04}-01-01", jahr);
    let jahr_end = format!("{:04}-12-31", jahr);

    let mut out = String::new();
    out.push_str("Datum;Typ;Beleg;Beschreibung;Kategorie;Netto;USt;Brutto;Vorsteuer\r\n");
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
                get_cents(r, 4)?,
                get_cents(r, 5)?,
                get_cents(r, 6)?,
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
            fmt_cents(netto),
            fmt_cents(ust),
            fmt_cents(brutto),
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
                get_cents(r, 4)?,
                get_cents(r, 5)?,
                get_cents(r, 6)?,
                r.get::<_, i64>(7)? != 0,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in exp_rows.flatten() {
        let (id, datum, kat, beschr, netto, ust, brutto, vsa) = row;
        let vorsteuer = if vsa { fmt_cents(ust) } else { String::new() };
        out.push_str(&format!(
            "{};Ausgabe;Ausgabe-{};{};{};{};{};{};{}\r\n",
            datum,
            id,
            csv_escape(&beschr),
            csv_escape(&kat),
            fmt_cents(netto),
            fmt_cents(ust),
            fmt_cents(brutto),
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
pub fn export_anlagen_csv(state: State<DbState>, path: String) -> Result<i64, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    let assets = list_assets_internal(&db).map_err(|e| e.to_string())?;
    let mut out = String::new();
    out.push_str(
        "Name;Inbetriebnahme;Anschaffung Netto;Anschaffung USt;Anschaffung Brutto;\
         Nutzungsdauer (Jahre);AfA pro Jahr;Notiz\r\n",
    );
    for a in &assets {
        let basis = a.anschaffung_netto + a.anschaffung_ust;
        // AfA pro Jahr aus der Basis (Cents) — Cents-Division mit Rundung.
        let afa_eur = (basis as f64) / (a.nutzungsdauer_jahre.max(1) as f64);
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{}\r\n",
            csv_escape(&a.name),
            a.inbetriebnahme,
            fmt_cents(a.anschaffung_netto),
            fmt_cents(a.anschaffung_ust),
            fmt_cents(basis),
            a.nutzungsdauer_jahre,
            fmt_f64(afa_eur / 100.0),
            csv_escape(a.notiz.as_deref().unwrap_or("")),
        ));
    }
    let mut bytes = Vec::with_capacity(out.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(out.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("Datei schreiben fehlgeschlagen: {}", e))?;
    Ok(assets.len() as i64)
}

// ── JSON Backup / Restore ───────────────────────────────────────────────────

/// Backup-Format v2: alle Geldbeträge in Cents (`i64`).
/// v1 (REAL €) wird beim Import via JSON-Walk migriert.
const BACKUP_VERSION: u32 = 2;

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
    #[serde(default)]
    vendor: Option<String>,
    #[serde(default)]
    anker_email: Option<String>,
    #[serde(default)]
    anker_password: Option<String>,
    #[serde(default)]
    anker_country: Option<String>,
    #[serde(default)]
    solaredge_api_key: Option<String>,
    #[serde(default)]
    solaredge_site_id: Option<String>,
}

fn collect_backup(conn: &Connection) -> Result<Backup, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh,
                speicher_laden_kwh, speicher_entladen_kwh, notiz
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
                speicher_laden_kwh: r.get(5)?,
                speicher_entladen_kwh: r.get(6)?,
                notiz: r.get(7)?,
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
                netto: get_cents(r, 4)?,
                ust: get_cents(r, 5)?,
                brutto: get_cents(r, 6)?,
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
                netto: get_cents(r, 4)?,
                ust: get_cents(r, 5)?,
                brutto: get_cents(r, 6)?,
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
        vendor: get_setting(conn, "vendor")?.filter(|s| !s.is_empty()),
        anker_email: get_setting(conn, "anker_email")?.filter(|s| !s.is_empty()),
        anker_password: get_setting(conn, "anker_password")?.filter(|s| !s.is_empty()),
        anker_country: get_setting(conn, "anker_country")?.filter(|s| !s.is_empty()),
        solaredge_api_key: get_setting(conn, "solaredge_api_key")?.filter(|s| !s.is_empty()),
        solaredge_site_id: get_setting(conn, "solaredge_site_id")?.filter(|s| !s.is_empty()),
    };

    Ok(Backup {
        version: BACKUP_VERSION,
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
pub fn export_backup(state: State<DbState>, path: String) -> Result<BackupSummary, String> {
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

/// Konvertiert ein v1-Backup (Geldbeträge als €-Float) in v2-Format (Cents-Int).
/// Arbeitet in-place auf dem geparsten JSON, bevor es als Backup-Struct deserialisiert wird.
fn migrate_backup_v1_to_v2(root: &mut serde_json::Value) {
    fn to_cents(v: &serde_json::Value) -> serde_json::Value {
        match v.as_f64() {
            Some(eur) => serde_json::json!((eur * 100.0).round() as i64),
            None => v.clone(),
        }
    }
    fn convert_field(item: &mut serde_json::Value, key: &str) {
        if let Some(obj) = item.as_object_mut() {
            if let Some(val) = obj.get(key) {
                let new = to_cents(val);
                obj.insert(key.to_string(), new);
            }
        }
    }
    fn convert_opt_field(item: &mut serde_json::Value, key: &str) {
        if let Some(obj) = item.as_object_mut() {
            if let Some(val) = obj.get(key) {
                if val.is_null() {
                    return;
                }
                let new = to_cents(val);
                obj.insert(key.to_string(), new);
            }
        }
    }
    fn walk(root: &mut serde_json::Value, key: &str, fields: &[&str]) {
        if let Some(arr) = root.get_mut(key).and_then(|v| v.as_array_mut()) {
            for item in arr {
                for f in fields {
                    convert_field(item, f);
                }
            }
        }
    }
    walk(root, "payouts", &["netto", "ust", "brutto"]);
    walk(root, "expenses", &["netto", "ust", "brutto"]);
    walk(root, "stromtarif_perioden", &["grundgebuehr_eur_per_monat"]);
    // Assets: zwei optionale Verkaufs-Felder zusätzlich.
    if let Some(arr) = root.get_mut("assets").and_then(|v| v.as_array_mut()) {
        for item in arr {
            convert_field(item, "anschaffung_netto");
            convert_field(item, "anschaffung_ust");
            convert_opt_field(item, "verkaufserloes_netto");
            convert_opt_field(item, "verkaufserloes_ust");
        }
    }
    if let Some(obj) = root.as_object_mut() {
        obj.insert("version".into(), serde_json::json!(BACKUP_VERSION));
    }
}

#[command]
pub fn import_backup(state: State<DbState>, path: String) -> Result<BackupSummary, String> {
    let json =
        std::fs::read_to_string(&path).map_err(|e| format!("Datei lesen fehlgeschlagen: {}", e))?;

    // Version-Check zuerst, dann ggf. v1 (€ als Float) → v2 (Cents als Int) migrieren.
    let mut raw: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("JSON-Format ungültig: {}", e))?;
    let version = raw
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    match version {
        1 => migrate_backup_v1_to_v2(&mut raw),
        2 => {}
        _ => {
            return Err(format!(
                "Unbekannte Backup-Version {} (erwartet 1 oder 2).",
                version
            ))
        }
    }
    let backup: Backup = serde_json::from_value(raw)
        .map_err(|e| format!("Backup-Struktur ungültig nach Migration: {}", e))?;

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
        "INSERT INTO settings (key, value) VALUES ('vendor', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.vendor.clone().unwrap_or_else(|| "none".to_string())],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('anker_email', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.anker_email.clone().unwrap_or_default()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('anker_password', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.anker_password.clone().unwrap_or_default()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('anker_country', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.anker_country.clone().unwrap_or_else(|| "DE".to_string())],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('solaredge_api_key', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.solaredge_api_key.clone().unwrap_or_default()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO settings (key, value) VALUES ('solaredge_site_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s.solaredge_site_id.clone().unwrap_or_default()],
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

// ── Vendor-API Import (Anker Solix Cloud via Python-Sidecar) ────────────────

#[derive(Serialize)]
pub struct ImportResult {
    pub imported: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub site_id: Option<String>,
}

#[derive(Deserialize)]
struct SidecarRow {
    date: String,
    erzeugung_kwh: f64,
    eigenverbrauch_kwh: f64,
    einspeisung_kwh: f64,
    #[serde(default)]
    netzbezug_kwh: Option<f64>,
    /// Solar -> Akku — nur fuer Anlagen mit Speicher (Anker Solarbank).
    #[serde(default)]
    speicher_laden_kwh: Option<f64>,
    /// Akku -> Haus.
    #[serde(default)]
    speicher_entladen_kwh: Option<f64>,
}

/// Streaming-NDJSON-Protokoll: jede Zeile auf stdout ist ein typed Event.
/// `kind`-Discriminator entscheidet wie Rust die Zeile verarbeitet.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SidecarLine {
    /// Anlagen-ID — kommt einmal vom Sidecar nach dem Login.
    Site { site_id: String },
    /// Ein Tages-Datensatz — Rust batched und committed.
    Row(SidecarRow),
    /// Tag uebersprungen (kein Insert) — z.B. erzeugung=0 oder Zukunft.
    Skip {
        #[allow(dead_code)]
        date: String,
        #[allow(dead_code)]
        reason: String,
    },
    /// Letzte Zeile mit gesammelten Warnungen.
    Summary {
        #[serde(default)]
        warnings: Vec<String>,
    },
}

#[derive(Deserialize)]
struct SidecarError {
    error: String,
}

/// Wieviele Rows in eine Transaktion. Bei Crash mid-stream sind die
/// vorherigen Batches sicher in der DB.
const COMMIT_BATCH_SIZE: usize = 30;

/// UPSERT eines Batches. Plausi-Checks pro Row, akkumuliert Stats.
fn commit_batch(
    state: &State<DbState>,
    batch: &mut Vec<SidecarRow>,
    imported: &mut i64,
    skipped: &mut i64,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    if batch.is_empty() {
        return Ok(());
    }
    let mut db = state.0.lock().map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for row in batch.iter() {
        if row.erzeugung_kwh <= 0.0 {
            *skipped += 1;
            errors.push(format!(
                "{}: erzeugung=0 — Schreib-Schutz aktiv, Tag uebersprungen.",
                row.date
            ));
            continue;
        }
        if row.einspeisung_kwh > row.erzeugung_kwh + 0.5 {
            *skipped += 1;
            errors.push(format!(
                "{}: Einspeisung ({:.2}) > Erzeugung ({:.2}) — uebersprungen.",
                row.date, row.einspeisung_kwh, row.erzeugung_kwh
            ));
            continue;
        }
        let res = tx.execute(
            "INSERT INTO daily_production
                (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh,
                 speicher_laden_kwh, speicher_entladen_kwh, notiz)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)
             ON CONFLICT(date) DO UPDATE SET
                erzeugung_kwh = excluded.erzeugung_kwh,
                eigenverbrauch_kwh = excluded.eigenverbrauch_kwh,
                einspeisung_kwh = excluded.einspeisung_kwh,
                netzbezug_kwh = excluded.netzbezug_kwh,
                -- Speicher-Felder nur ueberschreiben wenn der Sidecar
                -- konkret Werte liefert (NULL=keine Aenderung, sonst write).
                speicher_laden_kwh = COALESCE(excluded.speicher_laden_kwh, speicher_laden_kwh),
                speicher_entladen_kwh = COALESCE(excluded.speicher_entladen_kwh, speicher_entladen_kwh)",
            params![
                row.date,
                row.erzeugung_kwh,
                row.eigenverbrauch_kwh,
                row.einspeisung_kwh,
                row.netzbezug_kwh,
                row.speicher_laden_kwh,
                row.speicher_entladen_kwh,
            ],
        );
        match res {
            Ok(_) => *imported += 1,
            Err(e) => {
                *skipped += 1;
                errors.push(format!("{}: {}", row.date, e));
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    batch.clear();
    Ok(())
}

/// Beschreibt einen Vendor-Sidecar fuer den `resolve_sidecar`-Lookup.
/// Pro Vendor existiert ein eigenes Sidecar (Bin + venv) im Projekt.
struct SidecarSpec {
    /// Binary-Name ohne Suffix — wird sowohl als Plain-Name als auch mit
    /// `-<triple>(.exe)` neben der Executable gesucht.
    bin: &'static str,
    /// Ordnername unter dem Repo-Root — enthaelt main.py + optional .venv.
    repo_dir: &'static str,
}

fn spec_for(vendor: &str) -> Result<SidecarSpec, String> {
    match vendor {
        "anker" => Ok(SidecarSpec {
            bin: "anker-solix",
            repo_dir: "vendor-import-anker",
        }),
        "solaredge" => Ok(SidecarSpec {
            bin: "solaredge",
            repo_dir: "vendor-import-solaredge",
        }),
        "none" | "" => Err(
            "Kein Hersteller-API ausgewaehlt. Waehle Anker oder SolarEdge \
             unter Einstellungen → Hersteller-API."
                .into(),
        ),
        other => Err(format!("Unbekannter Vendor: '{other}'")),
    }
}

/// Resolves a vendor sidecar command. Tries (in order):
/// 1. `<exe_dir>/<bin>(.exe)` — production bundle (plain).
/// 2. `<exe_dir>/<bin>-<triple>(.exe)` — Tauri sidecar naming convention.
/// 3. `python3 <cwd>/<repo_dir>/main.py` — dev fallback. Bevorzugt
///    venv-Python falls vorhanden (Anker braucht Deps, SolarEdge nicht).
fn resolve_sidecar(spec: &SidecarSpec) -> Result<std::process::Command, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "current_exe hat kein Parent-Verzeichnis".to_string())?;
    let ext = if cfg!(windows) { ".exe" } else { "" };

    // Variante 1: nackter Name neben der Executable.
    let plain = exe_dir.join(format!("{}{ext}", spec.bin));
    if plain.is_file() {
        return Ok(std::process::Command::new(plain));
    }

    // Variante 2: Tauri-Sidecar-Naming mit Target-Triple-Suffix.
    let prefix = format!("{}-", spec.bin);
    if let Ok(entries) = std::fs::read_dir(exe_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(ext) {
                return Ok(std::process::Command::new(entry.path()));
            }
        }
    }

    // Variante 3: Dev-Fallback — Python-Script direkt im Repo aufrufen.
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let venv_python_rel: [&str; 3] = if cfg!(windows) {
        [".venv", "Scripts", "python.exe"]
    } else {
        [".venv", "bin", "python"]
    };
    for base in [cwd.clone(), cwd.join("..")] {
        let script = base.join(spec.repo_dir).join("main.py");
        if !script.is_file() {
            continue;
        }
        let mut venv_python = base.join(spec.repo_dir);
        for part in venv_python_rel {
            venv_python.push(part);
        }
        let interpreter = if venv_python.is_file() {
            venv_python.into_os_string()
        } else {
            std::ffi::OsString::from("python3")
        };
        let mut cmd = std::process::Command::new(interpreter);
        cmd.arg(script);
        return Ok(cmd);
    }

    Err(format!(
        "{} Sidecar nicht gefunden. Im Dev-Modus `python3 {}/main.py` testen, \
         fuer Release `./{}/build-sidecar.sh` ausfuehren.",
        spec.bin, spec.repo_dir, spec.repo_dir
    ))
}

#[command]
pub fn import_from_vendor(
    app: AppHandle,
    state: State<DbState>,
    von: String,
    bis: String,
) -> Result<ImportResult, String> {
    // Settings (Vendor-Wahl + Credentials) vor dem Sidecar-Aufruf lesen,
    // Lock dann freigeben (Sidecar laeuft 1-300s, blockiert sonst die DB).
    let (vendor, anker_email, anker_password, anker_country, se_api_key, se_site_id) = {
        let db = state.0.lock().map_err(|e| e.to_string())?;
        let vendor = get_setting(&db, "vendor")
            .map_err(|e| e.to_string())?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "none".to_string());
        let anker_email = get_setting(&db, "anker_email")
            .map_err(|e| e.to_string())?
            .filter(|s| !s.is_empty());
        let anker_password = get_setting(&db, "anker_password")
            .map_err(|e| e.to_string())?
            .filter(|s| !s.is_empty());
        let anker_country = get_setting(&db, "anker_country")
            .map_err(|e| e.to_string())?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "DE".to_string());
        let se_api_key = get_setting(&db, "solaredge_api_key")
            .map_err(|e| e.to_string())?
            .filter(|s| !s.is_empty());
        let se_site_id = get_setting(&db, "solaredge_site_id")
            .map_err(|e| e.to_string())?
            .filter(|s| !s.is_empty());
        (vendor, anker_email, anker_password, anker_country, se_api_key, se_site_id)
    };

    let spec = spec_for(&vendor)?;
    let mut cmd = resolve_sidecar(&spec)?;
    cmd.args(["--von", &von, "--bis", &bis])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Vendor-spezifische ENV — fehlende Credentials abfangen bevor Sidecar
    // startet, damit der User eine klare Meldung statt Stack-Trace bekommt.
    match vendor.as_str() {
        "anker" => {
            let (email, password) = match (anker_email, anker_password) {
                (Some(e), Some(p)) => (e, p),
                _ => {
                    return Err(
                        "Anker-Login fehlt. Hinterlege Email + Passwort unter \
                         Einstellungen → Hersteller-API."
                            .into(),
                    );
                }
            };
            cmd.env("ANKER_EMAIL", email)
                .env("ANKER_PASSWORD", password)
                .env("ANKER_COUNTRY", anker_country);
        }
        "solaredge" => {
            let (api_key, site_id) = match (se_api_key, se_site_id) {
                (Some(k), Some(s)) => (k, s),
                _ => {
                    return Err(
                        "SolarEdge-Zugang fehlt. Hinterlege API-Key + Site-ID \
                         unter Einstellungen → Hersteller-API."
                            .into(),
                    );
                }
            };
            cmd.env("SOLAREDGE_API_KEY", api_key)
                .env("SOLAREDGE_SITE_ID", site_id);
        }
        _ => unreachable!("spec_for hat bereits validiert"),
    };

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Sidecar-Start fehlgeschlagen: {e}"))?;

    // Stderr im Hintergrund-Thread lesen: NDJSON-Zeilen mit "progress"-Key
    // werden als Tauri-Event `anker-import-progress` an die UI gepusht;
    // alle anderen Zeilen (insb. das finale `{"error": "..."}`) sammeln wir
    // fuer das Error-Reporting nach dem Prozess-Exit.
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Sidecar hat keinen stderr-Pipe".to_string())?;
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let stderr_buf_thread = stderr_buf.clone();
    let app_thread = app.clone();
    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            // Try-parse als JSON; "progress" -> Event, sonst aufsammeln.
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if v.get("progress").is_some() {
                    let _ = app_thread.emit("anker-import-progress", &v);
                    continue;
                }
            }
            if let Ok(mut buf) = stderr_buf_thread.lock() {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
    });

    // Stdout streamend lesen: NDJSON Zeile fuer Zeile. Rows in Batches von
    // COMMIT_BATCH_SIZE committen, damit ein Crash mid-stream die bisherigen
    // Batches NICHT verliert (kann bei laufendem Anker-Import 5+ Min
    // wertvolle Daten retten).
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Sidecar hat keinen stdout-Pipe".to_string())?;
    let stdout_reader = BufReader::new(stdout);

    let mut site_id: Option<String> = None;
    let mut warnings: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut imported: i64 = 0;
    let mut skipped: i64 = 0;
    let mut batch: Vec<SidecarRow> = Vec::with_capacity(COMMIT_BATCH_SIZE);

    for line in stdout_reader.lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<SidecarLine>(line) {
            Ok(SidecarLine::Site { site_id: id }) => {
                site_id = Some(id);
            }
            Ok(SidecarLine::Row(row)) => {
                batch.push(row);
                if batch.len() >= COMMIT_BATCH_SIZE {
                    commit_batch(&state, &mut batch, &mut imported, &mut skipped, &mut errors)?;
                }
            }
            Ok(SidecarLine::Skip { date, reason }) => {
                skipped += 1;
                warnings.push(format!("{date}: {reason}"));
            }
            Ok(SidecarLine::Summary { warnings: w }) => {
                warnings.extend(w);
            }
            Err(e) => {
                errors.push(format!("Sidecar-Zeile nicht parsbar: {e}. Inhalt: {line}"));
            }
        }
    }

    // Finalen Batch committen, dann auf Sidecar-Exit warten.
    commit_batch(&state, &mut batch, &mut imported, &mut skipped, &mut errors)?;

    let status = child
        .wait()
        .map_err(|e| format!("Sidecar-Wait: {e}"))?;
    let _ = stderr_handle.join();
    let stderr_content = stderr_buf.lock().map(|s| s.clone()).unwrap_or_default();

    if !status.success() {
        // Sidecar serialisiert Fehler als JSON {"error": "..."} auf stderr.
        // Wenn wir aber schon Batches committed haben, bringen wir den
        // ImportResult mit der teilweisen Bilanz zurueck — die DB ist
        // konsistent, der User soll wissen was rein kam, bevor das Sidecar
        // umfiel.
        let sidecar_err = serde_json::from_str::<SidecarError>(stderr_content.trim())
            .map(|p| p.error)
            .unwrap_or_else(|_| {
                format!(
                    "Sidecar Exit-Code {:?}: {}",
                    status.code(),
                    stderr_content.trim()
                )
            });
        if imported == 0 {
            return Err(sidecar_err);
        }
        warnings.push(format!("Sidecar abgebrochen: {sidecar_err}"));
    }

    Ok(ImportResult {
        imported,
        skipped,
        errors,
        warnings,
        site_id,
    })
}
