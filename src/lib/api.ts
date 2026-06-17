import { invoke } from "@tauri-apps/api/core";
import type {
  Aggregat,
  Asset,
  DailyProduction,
  DashboardSnapshot,
  EuerReport,
  ExpectedEinspeisung,
  Expense,
  Payout,
  Periode,
  Settings,
  UstvaReport,
} from "./types";

/**
 * Browser-only Vorschau: Tauri-Window ist nicht verfügbar.
 * Die UI rendert trotzdem, aber API-Aufrufe schlagen mit verständlichem
 * Fehler fehl, statt die Konsole mit `__TAURI__ is not defined` zu fluten.
 */
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function ensureTauri(): void {
  if (!isTauri()) {
    throw new Error("Diese Funktion erfordert die Desktop-App.");
  }
}

/* ── Tageserfassung ──────────────────────────────────────────────────────── */

export async function listDailyRange(
  from: string,
  to: string,
): Promise<DailyProduction[]> {
  ensureTauri();
  return await invoke("list_daily_range", { from, to });
}

export async function getDaily(date: string): Promise<DailyProduction | null> {
  ensureTauri();
  return await invoke("get_daily", { date });
}

export async function upsertDaily(entry: DailyProduction): Promise<void> {
  ensureTauri();
  await invoke("upsert_daily", { entry });
}

export async function deleteDaily(date: string): Promise<void> {
  ensureTauri();
  await invoke("delete_daily", { date });
}

/* ── Auszahlungen (Bayernwerk) ───────────────────────────────────────────── */

export async function listPayouts(): Promise<Payout[]> {
  ensureTauri();
  return await invoke("list_payouts");
}

export async function upsertPayout(payout: Payout): Promise<number> {
  ensureTauri();
  return await invoke("upsert_payout", { payout });
}

export async function deletePayout(id: number): Promise<void> {
  ensureTauri();
  await invoke("delete_payout", { id });
}

/* ── Ausgaben ────────────────────────────────────────────────────────────── */

export async function listExpenses(): Promise<Expense[]> {
  ensureTauri();
  return await invoke("list_expenses");
}

export async function upsertExpense(expense: Expense): Promise<number> {
  ensureTauri();
  return await invoke("upsert_expense", { expense });
}

export async function deleteExpense(id: number): Promise<void> {
  ensureTauri();
  await invoke("delete_expense", { id });
}

/* ── Anlagen / AfA ───────────────────────────────────────────────────────── */

export async function listAssets(): Promise<Asset[]> {
  ensureTauri();
  return await invoke("list_assets");
}

export async function upsertAsset(asset: Asset): Promise<number> {
  ensureTauri();
  return await invoke("upsert_asset", { asset });
}

export async function deleteAsset(id: number): Promise<void> {
  ensureTauri();
  await invoke("delete_asset", { id });
}

/* ── Settings ────────────────────────────────────────────────────────────── */

export async function getSettings(): Promise<Settings> {
  ensureTauri();
  return await invoke("get_settings");
}

export async function setSettings(settings: Settings): Promise<void> {
  ensureTauri();
  await invoke("set_settings", { settings });
}

/* ── Gefahrenzone ──────────────────────────────────────────────────────── */

export interface WipeSummary {
  deleted_daily: number;
  deleted_payouts: number;
  deleted_expenses: number;
  deleted_assets: number;
  deleted_verlauf_eintraege: number;
}

/**
 * Loescht ALLE Nutzdaten irreversibel. Schema bleibt erhalten, Defaults
 * werden neu geseedet. `confirmationToken` muss exakt "WIPE" sein — sonst
 * lehnt Rust ab. UI fragt diesen Token via Input-Feld beim Nutzer ab.
 */
export async function wipeDatabase(
  confirmationToken: string,
): Promise<WipeSummary> {
  ensureTauri();
  return await invoke("wipe_database", { confirmationToken });
}

/* ── Reports / Statistik ─────────────────────────────────────────────────── */

export async function getDashboard(): Promise<DashboardSnapshot> {
  ensureTauri();
  return await invoke("get_dashboard");
}

export async function aggregate(
  periode: Periode,
  jahr: number | null,
): Promise<Aggregat[]> {
  ensureTauri();
  return await invoke("aggregate_production", { periode, jahr });
}

export async function getEuer(jahr: number): Promise<EuerReport> {
  ensureTauri();
  return await invoke("get_euer", { jahr });
}

export async function getUstva(
  jahr: number,
  monat: number | null,
): Promise<UstvaReport> {
  ensureTauri();
  return await invoke("get_ustva", { jahr, monat });
}

export async function getExpectedEinspeisung(
  jahr: number,
  monat: number | null,
): Promise<ExpectedEinspeisung> {
  ensureTauri();
  return await invoke("get_expected_einspeisung", { jahr, monat });
}

/* ── Export / Backup ─────────────────────────────────────────────────────── */

export interface BackupSummary {
  daily: number;
  payouts: number;
  expenses: number;
  assets: number;
}

export async function exportBuchungenCsv(
  path: string,
  jahr: number,
): Promise<number> {
  ensureTauri();
  return await invoke("export_buchungen_csv", { path, jahr });
}

export async function exportAnlagenCsv(path: string): Promise<number> {
  ensureTauri();
  return await invoke("export_anlagen_csv", { path });
}

export async function exportBackup(path: string): Promise<BackupSummary> {
  ensureTauri();
  return await invoke("export_backup", { path });
}

export async function importBackup(path: string): Promise<BackupSummary> {
  ensureTauri();
  return await invoke("import_backup", { path });
}

/* ── Anker Solix Cloud Import ────────────────────────────────────────────── */

export interface ImportResult {
  imported: number;
  skipped: number;
  errors: string[];
  warnings: string[];
  site_id: string | null;
}

/**
 * Importiert Tagesdaten aus der Anker-Solix-Cloud fuer den Zeitraum [von, bis].
 * Spawnt das Python-Sidecar `anker-solix`, das per inoffizieller API die
 * Tagesaggregate holt und als JSON liefert. Bestehende Tageseintraege werden
 * idempotent ueberschrieben (Anker liefert Tagessummen).
 */
export async function importFromVendor(
  von: string,
  bis: string,
): Promise<ImportResult> {
  ensureTauri();
  return await invoke("import_from_vendor", { von, bis });
}

export { isTauri };
