//! AfA-Mathematik: linear vs. GWG-Sofort, Sonder-AfA §7g Abs. 5 EStG, Verkauf.
//! Reine Funktionen auf `Asset` — keine DB, gut isoliert testbar.

use chrono::{Datelike, NaiveDate};

use crate::types::Asset;

/// Aktive AfA-Monate eines Jahres, abhängig von Inbetriebnahme und (optional)
/// Verkauf. Erstjahr ab Inbetriebnahmemonat, Letztjahr bis Verkaufsmonat
/// (Monat des Verkaufs zählt nicht mehr — übliche Praxis).
pub(crate) fn afa_monate(asset: &Asset, jahr: i32) -> i32 {
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0;
    };
    let start = if jahr == ib.year() {
        ib.month() as i32
    } else {
        1
    };
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

/// AfA-Basis in Cents = Anschaffungs-Netto + Anschaffungs-USt.
pub(crate) fn afa_basis(asset: &Asset) -> i64 {
    asset.anschaffung_netto + asset.anschaffung_ust
}

/// Reguläre AfA im Jahr — kombiniert lineare, GWG-Sofort und Verkauf.
/// Sonder-AfA wird separat in `sonder_afa_for_year` ausgewiesen.
/// Rückgabe in Cents, kaufmännisch gerundet.
pub(crate) fn afa_for_year(asset: &Asset, jahr: i32) -> i64 {
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0;
    };
    if asset.afa_methode == "gwg_sofort" {
        return if jahr == ib.year() {
            afa_basis(asset)
        } else {
            0
        };
    }
    // linear
    let nd = asset.nutzungsdauer_jahre.max(1) as f64;
    let yearly = afa_basis(asset) as f64 / nd;
    let monate = afa_monate(asset, jahr) as f64;
    if monate <= 0.0 {
        return 0;
    }
    let end_year = ib.year() + asset.nutzungsdauer_jahre as i32 - 1;
    if jahr > end_year {
        return 0;
    }
    (yearly * (monate / 12.0)).round() as i64
}

/// Sonder-AfA §7g Abs. 5 EStG — einmalig im Inbetriebnahmejahr, prozentual auf AK.
/// Rückgabe in Cents.
pub(crate) fn sonder_afa_for_year(asset: &Asset, jahr: i32) -> i64 {
    if asset.afa_methode == "gwg_sofort" {
        return 0;
    }
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0;
    };
    if jahr != ib.year() {
        return 0;
    }
    let pct = asset.sonderabschreibung_prozent.clamp(0.0, 50.0);
    (afa_basis(asset) as f64 * pct / 100.0).round() as i64
}

/// Kumulierte Abschreibung (linear + Sonder-AfA) bis einschließlich `jahr`.
fn afa_kumuliert_bis(asset: &Asset, jahr: i32) -> i64 {
    let Some(ib) = NaiveDate::parse_from_str(&asset.inbetriebnahme, "%Y-%m-%d").ok() else {
        return 0;
    };
    let mut sum: i64 = 0;
    for y in ib.year()..=jahr {
        sum += afa_for_year(asset, y) + sonder_afa_for_year(asset, y);
    }
    let basis = afa_basis(asset);
    if sum > basis {
        sum = basis;
    }
    sum
}

pub(crate) fn restbuchwert_bis(asset: &Asset, jahr: i32) -> i64 {
    (afa_basis(asset) - afa_kumuliert_bis(asset, jahr)).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `netto` in Euro; intern wird in Cents umgerechnet.
    fn asset_linear(inbetrieb: &str, netto_eur: i64, nd: i64) -> Asset {
        Asset {
            id: 1,
            name: "Test".into(),
            inbetriebnahme: inbetrieb.into(),
            anschaffung_netto: netto_eur * 100,
            anschaffung_ust: 0,
            nutzungsdauer_jahre: nd,
            afa_methode: "linear".into(),
            sonderabschreibung_prozent: 0.0,
            verkauft_am: None,
            verkaufserloes_netto: None,
            verkaufserloes_ust: None,
            notiz: None,
        }
    }

    /// Volljahr nach Inbetriebnahme: 12000 / 10 = 1200 €/Jahr = 120000 Cents.
    #[test]
    fn linear_volljahr() {
        let a = asset_linear("2020-01-01", 12000, 10);
        assert_eq!(afa_for_year(&a, 2021), 120_000);
        assert_eq!(afa_for_year(&a, 2029), 120_000); // 10. Jahr
        assert_eq!(afa_for_year(&a, 2030), 0); // ausgelaufen
    }

    /// Erstjahr pro-rata: Inbetriebnahme im April → 9 Monate.
    /// 12000 / 10 × 9/12 = 900 € = 90000 Cents.
    #[test]
    fn linear_erstjahr_prorata() {
        let a = asset_linear("2020-04-01", 12000, 10);
        assert_eq!(afa_for_year(&a, 2020), 90_000);
        assert_eq!(afa_for_year(&a, 2021), 120_000);
    }

    /// GWG-Sofort: voller Betrag im Erstjahr, danach 0.
    #[test]
    fn gwg_sofort_einmalig() {
        let mut a = asset_linear("2024-06-15", 800, 10);
        a.afa_methode = "gwg_sofort".into();
        assert_eq!(afa_for_year(&a, 2024), 80_000);
        assert_eq!(afa_for_year(&a, 2025), 0);
    }

    /// Verkauf Juli → 6 Monate AfA (Jan-Juni).
    /// 12000 / 10 × 6/12 = 600 € = 60000 Cents.
    #[test]
    fn linear_verkauf_stoppt_afa() {
        let mut a = asset_linear("2020-01-01", 12000, 10);
        a.verkauft_am = Some("2024-07-15".into());
        assert_eq!(afa_for_year(&a, 2024), 60_000);
        assert_eq!(afa_for_year(&a, 2025), 0);
    }

    /// Sonder-AfA §7g: 20% von 10000 = 2000 € = 200000 Cents.
    #[test]
    fn sonder_afa_nur_erstjahr() {
        let mut a = asset_linear("2024-01-01", 10000, 10);
        a.sonderabschreibung_prozent = 20.0;
        assert_eq!(sonder_afa_for_year(&a, 2024), 200_000);
        assert_eq!(sonder_afa_for_year(&a, 2025), 0);
    }

    #[test]
    fn sonder_afa_keine_bei_gwg() {
        let mut a = asset_linear("2024-01-01", 800, 10);
        a.afa_methode = "gwg_sofort".into();
        a.sonderabschreibung_prozent = 20.0;
        assert_eq!(sonder_afa_for_year(&a, 2024), 0);
    }

    /// Sonder-AfA auf 50% gedeckelt: 50% von 10000 = 5000 € = 500000 Cents.
    #[test]
    fn sonder_afa_deckelung() {
        let mut a = asset_linear("2024-01-01", 10000, 10);
        a.sonderabschreibung_prozent = 80.0;
        assert_eq!(sonder_afa_for_year(&a, 2024), 500_000);
    }

    /// Restbuchwert sinkt linear, ist nie negativ. Werte in Cents.
    #[test]
    fn restbuchwert_lauf() {
        let a = asset_linear("2020-01-01", 10000, 10);
        // AfA 1000/Jahr = 100000 Cents.
        assert_eq!(restbuchwert_bis(&a, 2020), 900_000);
        assert_eq!(restbuchwert_bis(&a, 2024), 500_000);
        assert_eq!(restbuchwert_bis(&a, 2029), 0);
        assert_eq!(restbuchwert_bis(&a, 2030), 0);
    }

    #[test]
    fn afa_monate_inbetrieb_monatsende() {
        let a = asset_linear("2024-03-30", 12000, 10);
        assert_eq!(afa_monate(&a, 2024), 10);
    }

    #[test]
    fn afa_vor_inbetrieb() {
        let a = asset_linear("2024-06-01", 12000, 10);
        assert_eq!(afa_monate(&a, 2023), 0);
        assert_eq!(afa_for_year(&a, 2023), 0);
    }
}
