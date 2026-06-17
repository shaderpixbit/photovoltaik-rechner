//! Reporting: Dashboard, EÜR, UStVA, erwartete Einspeisung, Aggregate.
//!
//! Die Logik lebt in `*_for(&Connection, …)`-Funktionen — die `#[command]`
//! Wrapper greifen nur den DbState-Mutex und rufen die Impl auf. Das gibt
//! uns ein Test-Seam: in-memory `Connection` reicht zum Verifizieren der
//! Steuer-Berechnungen ohne Tauri-State.
//!
//! Alle Geldbeträge sind in Cents (`i64`). Wo Rates × Mengen entstehen
//! (z.B. `kWh × €/kWh × USt-Satz`), wird in `f64` zwischengerechnet und auf
//! den ganzen Cent gerundet bevor es in die Cent-Summe fließt.

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{command, State};

use crate::afa::{afa_for_year, restbuchwert_bis, sonder_afa_for_year};
use crate::crud::list_assets_internal;
use crate::db::{get_setting_f64, DbState};
use crate::types::{
    round_to_cents, Aggregat, DailyProduction, DashboardSnapshot, EuerReport,
    ExpectedEinspeisung, UstvaReport,
};
use crate::verlauf::{
    betreiber_modus_for, load_betreiber_perioden, load_perioden, load_stromtarif_perioden,
    load_verguetung_perioden, modus_for, stromtarif_for, verguetung_for,
};

// ── Aggregations ────────────────────────────────────────────────────────────

pub(crate) fn aggregate_production_for(
    conn: &Connection,
    periode: &str,
    jahr: Option<i32>,
) -> Result<Vec<Aggregat>, String> {
    let (bucket_expr, where_clause, params_vec): (&str, String, Vec<String>) = match periode {
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
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
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

#[command]
pub fn aggregate_production(
    state: State<DbState>,
    periode: String,
    jahr: Option<i32>,
) -> Result<Vec<Aggregat>, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    aggregate_production_for(&db, &periode, jahr)
}

// ── Dashboard ───────────────────────────────────────────────────────────────

pub(crate) fn dashboard_for(
    conn: &Connection,
    today: NaiveDate,
) -> Result<DashboardSnapshot, String> {
    let today_iso = today.format("%Y-%m-%d").to_string();
    let woche_start = today - chrono::Duration::days(6);
    let woche_start_iso = woche_start.format("%Y-%m-%d").to_string();
    let monat_start = format!("{}-{:02}-01", today.year(), today.month());
    let jahr_start = format!("{}-01-01", today.year());

    let heute = conn
        .query_row(
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh,
                    speicher_laden_kwh, speicher_entladen_kwh, notiz
             FROM daily_production WHERE date = ?1",
            params![today_iso],
            |r| {
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
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let sum_kwh = |from: &str, to: &str| -> Result<f64, String> {
        conn.query_row(
            "SELECT COALESCE(SUM(erzeugung_kwh), 0) FROM daily_production
             WHERE date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())
    };
    let woche_kwh = sum_kwh(&woche_start_iso, &today_iso)?;
    let monat_kwh = sum_kwh(&monat_start, &today_iso)?;
    let jahr_kwh = sum_kwh(&jahr_start, &today_iso)?;

    let max_tag = conn
        .query_row(
            "SELECT date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh, netzbezug_kwh,
                    speicher_laden_kwh, speicher_entladen_kwh, notiz
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
                    speicher_laden_kwh: r.get(5)?,
                    speicher_entladen_kwh: r.get(6)?,
                    notiz: r.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let einnahmen_jahr: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(netto), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![jahr_start, today_iso],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?;

    let stromtarif_perioden = load_stromtarif_perioden(conn).map_err(|e| e.to_string())?;
    let bezugspreis_fallback = get_setting_f64(conn, "strom_bezugspreis", 0.35);
    // Taggenau aufsummieren als €, am Ende auf Cents runden.
    let einsparung_jahr: i64 = {
        let mut stmt = conn
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
        let mut sum_eur = 0.0_f64;
        for row in rows.flatten() {
            let (date, kwh) = row;
            let (arbeit, _) = stromtarif_for(&stromtarif_perioden, &date, bezugspreis_fallback);
            sum_eur += kwh * arbeit;
        }
        round_to_cents(sum_eur)
    };

    let betreiber_perioden = load_betreiber_perioden(conn).map_err(|e| e.to_string())?;
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

#[command]
pub fn get_dashboard(state: State<DbState>) -> Result<DashboardSnapshot, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    dashboard_for(&db, Local::now().date_naive())
}

// ── EÜR ─────────────────────────────────────────────────────────────────────

pub(crate) fn euer_for(conn: &Connection, jahr: i32) -> Result<EuerReport, String> {
    let perioden = load_perioden(conn).map_err(|e| e.to_string())?;
    let betreiber_perioden = load_betreiber_perioden(conn).map_err(|e| e.to_string())?;
    let ust_satz = get_setting_f64(conn, "ust_satz_regel", 0.19);
    let ev_preis = get_setting_f64(conn, "eigenverbrauch_preis", 0.20);

    let jahr_str = format!("{:04}", jahr);
    let jahr_start = format!("{}-01-01", jahr_str);
    let jahr_end = format!("{}-12-31", jahr_str);

    let einnahmen_einspeisung_netto: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(netto), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![jahr_start, jahr_end],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?;
    let einnahmen_ust_einsp: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![jahr_start, jahr_end],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?;

    // Eigenverbrauch (unentgeltliche Wertabgabe) — pro Tag den jeweils gültigen Modus.
    // Akkumulieren in € (f64), am Ende runden — entspricht "Rundung auf Periodensumme".
    let mut ev_netto_eur = 0.0_f64;
    let mut ev_ust_eur = 0.0_f64;
    {
        let mut stmt = conn
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
            ev_netto_eur += netto;
            if modus == "regel" {
                ev_ust_eur += netto * ust_satz;
            }
        }
    }
    let einnahmen_eigenverbrauch_netto = round_to_cents(ev_netto_eur);
    let ev_ust_cents = round_to_cents(ev_ust_eur);

    let (ausgaben_betrieb_netto, ausgaben_betrieb_ust): (i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(netto), 0), COALESCE(SUM(ust), 0) FROM expenses
             WHERE date BETWEEN ?1 AND ?2",
            params![jahr_start, jahr_end],
            |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?)),
        )
        .map(|(n, u)| (n.round() as i64, u.round() as i64))
        .map_err(|e| e.to_string())?;

    let vorsteuer: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM expenses
             WHERE date BETWEEN ?1 AND ?2 AND vorsteuer_abzugsfaehig = 1",
            params![jahr_start, jahr_end],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?;

    let assets = list_assets_internal(conn).map_err(|e| e.to_string())?;
    let ausgaben_afa: i64 = assets.iter().map(|a| afa_for_year(a, jahr)).sum();
    let ausgaben_sonder_afa: i64 = assets.iter().map(|a| sonder_afa_for_year(a, jahr)).sum();

    // Anlagenverkäufe im Jahr: Erlös netto (Einnahme), Restbuchwert zum
    // Beginn des Verkaufsjahres als Aufwand (Abgang). Differenz = Veräußerungs-
    // gewinn/-verlust und fließt automatisch in den Gewinn.
    let mut einnahmen_veraeusserung_netto: i64 = 0;
    let mut ausgaben_restbuchwert_abgang: i64 = 0;
    for a in &assets {
        if let Some(va) = a
            .verkauft_am
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        {
            if va.year() == jahr {
                einnahmen_veraeusserung_netto += a.verkaufserloes_netto.unwrap_or(0);
                ausgaben_restbuchwert_abgang += restbuchwert_bis(a, jahr);
            }
        }
    }

    let einnahmen_ust = einnahmen_ust_einsp + ev_ust_cents;
    let einnahmen =
        einnahmen_einspeisung_netto + einnahmen_eigenverbrauch_netto + einnahmen_veraeusserung_netto;
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
        einnahmen_eigenverbrauch_netto,
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

#[command]
pub fn get_euer(state: State<DbState>, jahr: i32) -> Result<EuerReport, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    euer_for(&db, jahr)
}

// ── UStVA ───────────────────────────────────────────────────────────────────

fn periode_range(jahr: i32, monat: Option<i32>) -> Result<(String, String), String> {
    match monat {
        Some(m) => {
            let start = NaiveDate::from_ymd_opt(jahr, m as u32, 1)
                .ok_or_else(|| format!("Ungültiger Monat: {}/{}", m, jahr))?;
            let next = if m == 12 {
                NaiveDate::from_ymd_opt(jahr + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(jahr, (m + 1) as u32, 1).unwrap()
            };
            let end = next - chrono::Duration::days(1);
            Ok((
                start.format("%Y-%m-%d").to_string(),
                end.format("%Y-%m-%d").to_string(),
            ))
        }
        None => Ok((format!("{:04}-01-01", jahr), format!("{:04}-12-31", jahr))),
    }
}

pub(crate) fn ustva_for(
    conn: &Connection,
    jahr: i32,
    monat: Option<i32>,
) -> Result<UstvaReport, String> {
    let perioden = load_perioden(conn).map_err(|e| e.to_string())?;
    let ust_satz = get_setting_f64(conn, "ust_satz_regel", 0.19);
    let ev_preis = get_setting_f64(conn, "eigenverbrauch_preis", 0.20);

    let (from, to) = periode_range(jahr, monat)?;

    // Aktiver Modus = Modus am Periodenende (Stichtag).
    let modus = modus_for(&perioden, &to);

    let ust_einnahmen_payouts: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM payouts
             WHERE buchung_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?;
    let ust_einnahmen_verkauf: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(verkaufserloes_ust), 0) FROM assets
             WHERE verkauft_am IS NOT NULL AND verkauft_am BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?;
    let ust_einnahmen = ust_einnahmen_payouts + ust_einnahmen_verkauf;

    let ev_kwh: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(eigenverbrauch_kwh), 0) FROM daily_production
             WHERE date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map_err(|e| e.to_string())?;
    let ust_eigenverbrauch: i64 = if modus == "regel" {
        round_to_cents(ev_kwh * ev_preis * ust_satz)
    } else {
        0
    };

    let vorsteuer: i64 = if modus == "regel" {
        conn.query_row(
            "SELECT COALESCE(SUM(ust), 0) FROM expenses
             WHERE date BETWEEN ?1 AND ?2 AND vorsteuer_abzugsfaehig = 1",
            params![from, to],
            |r| r.get::<_, f64>(0),
        )
        .map(|v| v.round() as i64)
        .map_err(|e| e.to_string())?
    } else {
        0
    };

    // Kleinunternehmer erhebt keine USt und führt keine ab.
    let (ust_einnahmen, ust_eigenverbrauch, vorsteuer) = if modus == "kleinunternehmer" {
        (0, 0, 0)
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

#[command]
pub fn get_ustva(
    state: State<DbState>,
    jahr: i32,
    monat: Option<i32>,
) -> Result<UstvaReport, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    ustva_for(&db, jahr, monat)
}

// ── Erwartete Einspeisevergütung ────────────────────────────────────────────

pub(crate) fn expected_einspeisung_for(
    conn: &Connection,
    jahr: i32,
    monat: Option<i32>,
) -> Result<ExpectedEinspeisung, String> {
    let verguetung_perioden = load_verguetung_perioden(conn).map_err(|e| e.to_string())?;
    let (from, to) = periode_range(jahr, monat)?;

    let mut stmt = conn
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
    let mut erwartet_netto_eur = 0.0_f64;
    let mut tage_ohne_satz: i64 = 0;
    for row in rows.flatten() {
        let (date, kwh) = row;
        kwh_sum += kwh;
        match verguetung_for(&verguetung_perioden, &date) {
            Some((satz_eur, _modell)) => erwartet_netto_eur += kwh * satz_eur,
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
        erwartet_netto: round_to_cents(erwartet_netto_eur),
        tage_ohne_satz,
    })
}

#[command]
pub fn get_expected_einspeisung(
    state: State<DbState>,
    jahr: i32,
    monat: Option<i32>,
) -> Result<ExpectedEinspeisung, String> {
    let db = state.0.lock().map_err(|e| e.to_string())?;
    expected_einspeisung_for(&db, jahr, monat)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_schema, seed_defaults};

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        seed_defaults(&conn).unwrap();
        conn
    }

    fn set_kv(conn: &Connection, key: &str, val: &str) {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, val],
        )
        .unwrap();
    }

    /// Regelbesteuerung 2024: Einspeisung 80€ netto + 15.20€ USt, 500 kWh
    /// Eigenverbrauch à 20 ct = 100€ netto + 19€ USt, 100€ Ausgabe mit 19€ VSt.
    /// Erwartet: Einsp 8000, EV 10000, USt total 1520+1900=3420, VSt 1900,
    /// Gewinn = 8000+10000 - 10000 = 8000 Cents.
    #[test]
    fn euer_regel_einfach() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO payouts (buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto, kwh)
             VALUES ('2024-03-15', '2024-01-01', '2024-03-31', 8000, 1520, 9520, 1000.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-06-15', 1500.0, 500.0, 1000.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO expenses (date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig)
             VALUES ('2024-04-10', 'Wartung', 'Reinigung', 10000, 1900, 11900, 1)",
            [],
        )
        .unwrap();

        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.einnahmen_einspeisung_netto, 8000);
        assert_eq!(r.einnahmen_eigenverbrauch_netto, 10000);
        assert_eq!(r.einnahmen_ust, 1520 + 1900);
        assert_eq!(r.ausgaben_betrieb_netto, 10000);
        assert_eq!(r.vorsteuer, 1900);
        assert_eq!(r.gewinn_vor_steuern, 8000);
        assert_eq!(r.betreiber_modus, "gewerblich");
        assert!(r.est_pflichtig);
    }

    #[test]
    fn euer_kleinunternehmer_keine_ust() {
        let conn = fresh_db();
        conn.execute("DELETE FROM ust_perioden", []).unwrap();
        conn.execute(
            "INSERT INTO ust_perioden (effective_from, modus)
             VALUES ('2024-01-01', 'kleinunternehmer')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-06-15', 1500.0, 500.0, 1000.0)",
            [],
        )
        .unwrap();
        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.einnahmen_eigenverbrauch_netto, 10000);
        assert_eq!(r.einnahmen_ust, 0);
    }

    #[test]
    fn euer_nullsteuer_keine_ev_ust() {
        let conn = fresh_db();
        conn.execute("DELETE FROM ust_perioden", []).unwrap();
        conn.execute(
            "INSERT INTO ust_perioden (effective_from, modus)
             VALUES ('2024-01-01', 'nullsteuer')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payouts (buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto)
             VALUES ('2024-03-15', '2024-01-01', '2024-03-31', 8000, 1520, 9520)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-06-15', 1500.0, 500.0, 1000.0)",
            [],
        )
        .unwrap();
        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.einnahmen_ust, 1520);
        assert_eq!(r.einnahmen_eigenverbrauch_netto, 10000);
    }

    #[test]
    fn euer_privat_nicht_est_pflichtig() {
        let conn = fresh_db();
        conn.execute("DELETE FROM betreiber_perioden", []).unwrap();
        conn.execute(
            "INSERT INTO betreiber_perioden (effective_from, modus)
             VALUES ('2024-01-01', 'privat')",
            [],
        )
        .unwrap();
        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.betreiber_modus, "privat");
        assert!(!r.est_pflichtig);
        assert!(r.est_befreiungsgrund.is_some());
    }

    /// UStVA Juni 2024: USt-Einnahmen 200€, 100 kWh EV à 20ct × 19% = 3.80€,
    /// VSt 50€. Zahllast = 20000 + 380 - 5000 = 15380 Cents.
    #[test]
    fn ustva_monatlich_regel() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO payouts (buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto)
             VALUES ('2024-06-20', '2024-06-01', '2024-06-30', 105300, 20000, 125300)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payouts (buchung_date, zeitraum_von, zeitraum_bis, netto, ust, brutto)
             VALUES ('2024-05-20', '2024-05-01', '2024-05-31', 50000, 9500, 59500)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-06-15', 300.0, 100.0, 200.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO expenses (date, kategorie, beschreibung, netto, ust, brutto, vorsteuer_abzugsfaehig)
             VALUES ('2024-06-05', 'Wartung', 'Service', 26316, 5000, 31316, 1)",
            [],
        )
        .unwrap();
        let r = ustva_for(&conn, 2024, Some(6)).unwrap();
        assert_eq!(r.modus, "regel");
        assert_eq!(r.ust_einnahmen, 20000);
        assert_eq!(r.ust_eigenverbrauch, 380);
        assert_eq!(r.vorsteuer, 5000);
        assert_eq!(r.zahllast, 15_380);
    }

    /// Anlagenverkauf mid-2024: AfA Jan-Jun, Restbuchwert Abgang, Verkaufserlös.
    /// 12000€ über 10 Jahre, IB 2020-01-01. Kumuliert bis Ende 2023: 4×1200=4800.
    /// 2024 AfA (6 Mon): 600. RBW = 12000-4800-600 = 6600. Erlös 8000.
    /// In Cents.
    #[test]
    fn euer_anlagenverkauf() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO assets (name, inbetriebnahme, anschaffung_netto, anschaffung_ust,
                                 nutzungsdauer_jahre, afa_methode, sonderabschreibung_prozent,
                                 verkauft_am, verkaufserloes_netto, verkaufserloes_ust)
             VALUES ('Modul', '2020-01-01', 1200000, 0, 10, 'linear', 0.0,
                     '2024-07-15', 800000, 0)",
            [],
        )
        .unwrap();
        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.ausgaben_afa, 60_000);
        assert_eq!(r.einnahmen_veraeusserung_netto, 800_000);
        assert_eq!(r.ausgaben_restbuchwert_abgang, 660_000);
    }

    /// Vergütungs-Verlauf taggenau. 100 kWh × 8 ct + 100 kWh × 7 ct = 15.00€ = 1500.
    #[test]
    fn expected_einspeisung_taggenau() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO verguetung_perioden (effective_from, modell, satz_ct_per_kwh)
             VALUES ('2024-01-01', 'ueberschuss', 8.0),
                    ('2024-04-01', 'ueberschuss', 7.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-03-15', 200.0, 100.0, 100.0),
                    ('2024-05-15', 200.0, 100.0, 100.0)",
            [],
        )
        .unwrap();
        let r = expected_einspeisung_for(&conn, 2024, None).unwrap();
        assert_eq!(r.erwartet_netto, 1500);
        assert_eq!(r.kwh, 200.0);
        assert_eq!(r.tage_ohne_satz, 0);
    }

    /// Stromtarif taggenau: 100 × 0.30 + 100 × 0.40 = 70€ = 7000 Cents.
    #[test]
    fn dashboard_einsparung_taggenau() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO stromtarif_perioden
             (effective_from, arbeitspreis_eur_per_kwh, grundgebuehr_eur_per_monat)
             VALUES ('2024-01-01', 0.30, 0),
                    ('2024-07-01', 0.40, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-03-15', 0.0, 100.0, 0.0),
                    ('2024-08-15', 0.0, 100.0, 0.0)",
            [],
        )
        .unwrap();
        let today = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let d = dashboard_for(&conn, today).unwrap();
        assert_eq!(d.einsparung_jahr, 7000);
    }

    #[test]
    fn aggregate_monat() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-03-01', 10.0, 5.0, 5.0),
                    ('2024-03-02', 20.0, 10.0, 10.0),
                    ('2024-04-01', 15.0, 7.0, 8.0)",
            [],
        )
        .unwrap();
        let aggs = aggregate_production_for(&conn, "monat", Some(2024)).unwrap();
        assert_eq!(aggs.len(), 2);
        assert_eq!(aggs[0].bucket, "2024-03");
        assert_eq!(aggs[0].erzeugung_kwh, 30.0);
        assert_eq!(aggs[1].bucket, "2024-04");
        assert_eq!(aggs[1].erzeugung_kwh, 15.0);
    }

    /// USt-Wechsel mid-year. Feb (regel) → EV-USt; Juli (kleinunternehmer) → 0.
    /// EV-Netto beide Tage: 2 × 100 × 0.20 = 40€ = 4000 Cents.
    /// EV-USt nur Feb: 100 × 0.20 × 0.19 = 3.80€ = 380.
    #[test]
    fn euer_ust_modus_wechsel_mid_year() {
        let conn = fresh_db();
        conn.execute("DELETE FROM ust_perioden", []).unwrap();
        conn.execute(
            "INSERT INTO ust_perioden (effective_from, modus)
             VALUES ('2024-01-01', 'regel'),
                    ('2024-06-01', 'kleinunternehmer')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-02-15', 200.0, 100.0, 100.0),
                    ('2024-07-15', 200.0, 100.0, 100.0)",
            [],
        )
        .unwrap();
        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.einnahmen_eigenverbrauch_netto, 4000);
        assert_eq!(r.einnahmen_ust, 380);
    }

    /// EV-Preis 25 ct → 100 × 0.25 = 25€ = 2500. USt: 100 × 0.25 × 0.19 = 4.75 = 475.
    #[test]
    fn euer_respektiert_ev_preis_setting() {
        let conn = fresh_db();
        set_kv(&conn, "eigenverbrauch_preis", "0.25");
        conn.execute(
            "INSERT INTO daily_production (date, erzeugung_kwh, eigenverbrauch_kwh, einspeisung_kwh)
             VALUES ('2024-06-15', 0.0, 100.0, 0.0)",
            [],
        )
        .unwrap();
        let r = euer_for(&conn, 2024).unwrap();
        assert_eq!(r.einnahmen_eigenverbrauch_netto, 2500);
        assert_eq!(r.einnahmen_ust, 475);
    }
}
