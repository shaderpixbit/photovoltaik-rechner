//! Photovoltaik — Tauri-Backend.
//!
//! Aufgeteilt in:
//! - `types`   — Serde-Structs (spiegeln `src/lib/types.ts`)
//! - `db`      — Connection, Schema, Migrationen, Settings-KV-Helper
//! - `verlauf` — Verlaufstabellen (USt/Betreiber/Vergütung/Stromtarif)
//! - `afa`     — reine AfA-Mathematik
//! - `crud`    — CRUD-Commands je Entität
//! - `reports` — Dashboard, EÜR, UStVA, erwartete Einspeisung, Aggregate
//! - `exports` — CSV, JSON-Backup, Vendor-Stub

mod afa;
mod crud;
mod db;
mod exports;
mod reports;
mod types;
mod verlauf;

pub use db::{open_db, DbState};

use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = open_db().expect("Failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            crud::list_daily_range,
            crud::get_daily,
            crud::upsert_daily,
            crud::delete_daily,
            crud::list_payouts,
            crud::upsert_payout,
            crud::delete_payout,
            crud::list_expenses,
            crud::upsert_expense,
            crud::delete_expense,
            crud::list_assets,
            crud::upsert_asset,
            crud::delete_asset,
            crud::get_settings,
            crud::set_settings,
            reports::aggregate_production,
            reports::get_dashboard,
            reports::get_euer,
            reports::get_ustva,
            reports::get_expected_einspeisung,
            exports::export_buchungen_csv,
            exports::export_anlagen_csv,
            exports::export_backup,
            exports::import_backup,
            exports::import_from_vendor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
