//! Verlaufs-Tabellen (USt-Modus / Betreiber-Status / Vergütungssatz / Stromtarif).
//!
//! Drei orthogonale Achsen + Stromtarif: jede Buchung wählt sich den am Tagesdatum
//! gültigen Eintrag (`effective_from` ≤ date). Die Tabellen werden gemeinsam mit
//! den Settings geladen und in die EÜR-/UStVA-/Dashboard-Berechnung gefüttert.

use rusqlite::Connection;

use crate::db::get_cents;
use crate::types::{BetreiberPeriode, StromtarifPeriode, UstPeriode, VerguetungPeriode};

pub(crate) fn load_perioden(conn: &Connection) -> Result<Vec<UstPeriode>, rusqlite::Error> {
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

pub(crate) fn load_betreiber_perioden(
    conn: &Connection,
) -> Result<Vec<BetreiberPeriode>, rusqlite::Error> {
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

pub(crate) fn load_verguetung_perioden(
    conn: &Connection,
) -> Result<Vec<VerguetungPeriode>, rusqlite::Error> {
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

pub(crate) fn load_stromtarif_perioden(
    conn: &Connection,
) -> Result<Vec<StromtarifPeriode>, rusqlite::Error> {
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
                grundgebuehr_eur_per_monat: get_cents(r, 3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Liefert Arbeitspreis (€/kWh, `f64`) und Grundgebühr (Cents/Monat, `i64`) am Stichtag.
/// Fällt auf das Setting `strom_bezugspreis` zurück, wenn kein Eintrag existiert.
pub(crate) fn stromtarif_for(
    perioden: &[StromtarifPeriode],
    date: &str,
    fallback_arbeitspreis: f64,
) -> (f64, i64) {
    let mut arbeit = fallback_arbeitspreis;
    let mut grund: i64 = 0;
    let mut hit = false;
    for p in perioden {
        if p.effective_from.as_str() <= date {
            arbeit = p.arbeitspreis_eur_per_kwh;
            grund = p.grundgebuehr_eur_per_monat;
            hit = true;
        }
    }
    if !hit {
        (fallback_arbeitspreis, 0)
    } else {
        (arbeit, grund)
    }
}

/// Picks the modus active on `date` (the latest period whose effective_from ≤ date).
pub(crate) fn modus_for(perioden: &[UstPeriode], date: &str) -> String {
    let mut chosen = "regel".to_string();
    for p in perioden {
        if p.effective_from.as_str() <= date {
            chosen = p.modus.clone();
        }
    }
    chosen
}

pub(crate) fn betreiber_modus_for(perioden: &[BetreiberPeriode], date: &str) -> String {
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
pub(crate) fn verguetung_for(
    perioden: &[VerguetungPeriode],
    date: &str,
) -> Option<(f64, String)> {
    let mut chosen: Option<(f64, String)> = None;
    for p in perioden {
        if p.effective_from.as_str() <= date {
            chosen = Some((p.satz_ct_per_kwh / 100.0, p.modell.clone()));
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ust(id: i64, from: &str, modus: &str) -> UstPeriode {
        UstPeriode {
            id,
            effective_from: from.into(),
            modus: modus.into(),
        }
    }
    fn vp(id: i64, from: &str, modell: &str, ct: f64) -> VerguetungPeriode {
        VerguetungPeriode {
            id,
            effective_from: from.into(),
            modell: modell.into(),
            satz_ct_per_kwh: ct,
        }
    }
    fn st(id: i64, from: &str, arbeit: f64, grund_cents: i64) -> StromtarifPeriode {
        StromtarifPeriode {
            id,
            effective_from: from.into(),
            arbeitspreis_eur_per_kwh: arbeit,
            grundgebuehr_eur_per_monat: grund_cents,
        }
    }

    #[test]
    fn modus_picks_latest_effective() {
        let perioden = vec![
            ust(1, "2020-01-01", "regel"),
            ust(2, "2024-06-01", "kleinunternehmer"),
        ];
        assert_eq!(modus_for(&perioden, "2020-01-01"), "regel");
        assert_eq!(modus_for(&perioden, "2024-05-31"), "regel");
        assert_eq!(modus_for(&perioden, "2024-06-01"), "kleinunternehmer");
        assert_eq!(modus_for(&perioden, "2025-12-31"), "kleinunternehmer");
    }

    #[test]
    fn modus_default_regel_wenn_kein_eintrag_passt() {
        let perioden = vec![ust(1, "2024-06-01", "kleinunternehmer")];
        // Vor erstem Eintrag → Default "regel".
        assert_eq!(modus_for(&perioden, "2024-01-01"), "regel");
    }

    #[test]
    fn verguetung_none_vor_erstem_eintrag() {
        let perioden = vec![vp(1, "2024-01-01", "ueberschuss", 8.2)];
        assert!(verguetung_for(&perioden, "2023-12-31").is_none());
        let v = verguetung_for(&perioden, "2024-06-15").unwrap();
        assert!((v.0 - 0.082).abs() < 1e-6, "ct → €/kWh");
        assert_eq!(v.1, "ueberschuss");
    }

    #[test]
    fn stromtarif_fallback_ohne_eintrag() {
        let (arbeit, grund) = stromtarif_for(&[], "2024-06-15", 0.35);
        assert_eq!(arbeit, 0.35);
        assert_eq!(grund, 0);
    }

    /// Grundgebühr in Cents (1000 = 10€/Monat).
    #[test]
    fn stromtarif_taggenau_wechsel() {
        let perioden = vec![
            st(1, "2024-01-01", 0.30, 0),
            st(2, "2024-07-01", 0.40, 1000),
        ];
        assert_eq!(stromtarif_for(&perioden, "2024-03-15", 0.99), (0.30, 0));
        assert_eq!(stromtarif_for(&perioden, "2024-07-01", 0.99), (0.40, 1000));
        assert_eq!(stromtarif_for(&perioden, "2024-12-31", 0.99), (0.40, 1000));
    }

    /// Vor dem ersten effective_from greift der Fallback, nicht der erste Eintrag.
    #[test]
    fn stromtarif_vor_erstem_eintrag_fallback() {
        let perioden = vec![st(1, "2024-07-01", 0.40, 0)];
        assert_eq!(stromtarif_for(&perioden, "2024-01-15", 0.35), (0.35, 0));
    }
}
