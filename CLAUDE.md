# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Selbst gehosteter Photovoltaik-Manager im Troiber-Style (vgl. `ampel-ramp`,
`troiber-labelprint`). Verwaltet Tageserzeugung, Eigenverbrauch, Einspeisung,
Bayernwerk-Auszahlungen, Betriebsausgaben, Anlagen mit linearer AfA und
berechnet EÜR + UStVA für das Finanzamt.

User-facing strings sind deutsch.

## Commands

```bash
bun install
bun run tauri dev          # full app (Vite :1420 + Tauri-Fenster)
bun run dev                # nur Frontend, alle API-Calls werfen "erfordert Desktop-App"
bun run check              # svelte-kit sync && svelte-check (typecheck-Gate vor Commits)
bun run tauri build        # Release-Installer
```

## Architecture

### Stack
- Tauri 2 + SvelteKit 2 (`adapter-static`, kein SSR) + Svelte 5 (runes only)
- TailwindCSS 4 via `@tailwindcss/vite`, Troiber-Tokens in `src/app.css`
- shadcn-svelte-Konventionen (`components.json`) — die UI-Primitives in
  `src/lib/components/ui/` sind handgeschrieben (Button, Card, Input, Label, Select),
  passen aber zu späteren `bunx shadcn-svelte add …`-Komponenten.
- Bun als Package-Manager.

### Datenfluss
SQLite-Datenbank `photovoltaik.db` neben der Executable, WAL-Journal (lokale
App, kein SMB-Mehrclient-Szenario wie bei `ampel-ramp`). Frontend ruft Tauri-
Commands ausschließlich über `src/lib/api.ts` (typed wrappers + `ensureTauri()`).
`src/lib/types.ts` spiegelt die Rust-Structs feldweise — bei Änderung beide
Dateien synchron halten, es gibt kein Codegen.

### Schema (`src-tauri/src/lib.rs`)
| Tabelle | Inhalt |
|---|---|
| `daily_production` | Tagessumme: `date` (PK), Erzeugung, Eigenverbrauch, Einspeisung, optional Netzbezug, Notiz |
| `payouts` | Bayernwerk-Gutschriften: Buchungsdatum, Zeitraum von/bis, netto, USt, brutto, kWh |
| `expenses` | Betriebsausgaben mit Kategorie und Vorsteuer-Flag |
| `assets` | Anlagengüter für AfA: Inbetriebnahme, netto, USt, Nutzungsdauer |
| `ust_perioden` | Verlauf USt-Modus (`regel` / `kleinunternehmer` / `nullsteuer`) |
| `betreiber_perioden` | Verlauf Betreiber-Status (`gewerblich` / `privat` = §3 Nr. 72 EStG) |
| `verguetung_perioden` | Verlauf Einspeisevergütung: `effective_from`, `modell` (`ueberschuss` / `voll` / `direktvermarktung`), `satz_ct_per_kwh` |
| `settings` | Key-Value: `ust_satz_regel`, `eigenverbrauch_preis`, `strom_bezugspreis`, `anker_api_url`, `anker_api_token` |

Schema-Migrationen laufen idempotent in `create_schema()` beim App-Start
(`CREATE TABLE IF NOT EXISTS …`, additive Spalten via `add_column_if_missing`).
Es gibt kein separates Migration-Tool.

### Drei orthogonale Verlaufs-Achsen
Status ist immer eine **Verlaufstabelle**, nicht ein einzelner Schalter — die
App wählt für jede Buchung den am Tagesdatum gültigen Eintrag. Helper:
`modus_for` / `betreiber_modus_for` / `verguetung_for` in `lib.rs`.

**USt-Modus** (`ust_perioden`) — wirkt auf UStVA + Vorsteuer:
- `regel` — 19% USt auf Einspeisung, Eigenverbrauch wird als unentgeltliche
  Wertabgabe besteuert (kWh × Eigenverbrauchspreis × 19%), Vorsteuer abziehbar.
- `kleinunternehmer` — §19 UStG, keine USt-Berechnung, keine Vorsteuer.
- `nullsteuer` — §12(3) UStG ab 2023: 0% USt auf Anschaffung der Anlage, aber
  Einspeisung bleibt regelbesteuert. Die Eigenverbrauchsbesteuerung entfällt seit
  dem BMF-Schreiben vom 27.02.2023 — im Code: kein EV-USt-Anteil in der UStVA.

**Betreiber-Status** (`betreiber_perioden`) — wirkt auf ESt-Pflicht:
- `gewerblich` — voll EÜR-pflichtig, AfA fließt in die Bemessungsgrundlage.
- `privat` — §3 Nr. 72 EStG (PV ≤30 kWp bzw. 15 kWp je Wohneinheit,
  einkommensteuerbefreit). EÜR-Werte werden weiter berechnet, aber
  `est_pflichtig=false` + `est_befreiungsgrund` setzen — UI zeigt Banner.

**Vergütungssatz** (`verguetung_perioden`) — wirkt auf erwartete
Einspeisevergütung. `get_expected_einspeisung(jahr, monat?)` summiert
`Σ einspeisung_kwh × satz_eur` taggenau aus dem Verlauf; ohne hinterlegten
Satz für einen Tag mit Einspeisung → `tage_ohne_satz` als Hinweis.

EÜR (`get_euer`) wertet pro Tag den jeweils gültigen USt-Modus aus und nimmt
den Betreiber-Status am Jahresende. UStVA (`get_ustva`) verwendet den USt-Modus
am Periodenende.

### AfA und Anlagenverkauf
`afa_for_year()` + `sonder_afa_for_year()` in `src-tauri/src/lib.rs`:
AfA-Basis = `anschaffung_netto + anschaffung_ust` (im Nullsteuer-Fall
USt = 0 in `assets`). Drei Aspekte:

- **Methode** (`assets.afa_methode`): `linear` (default) oder `gwg_sofort`.
  Linear: AfA = Basis / Nutzungsdauer, im Erstjahr pro-rata-temporis ab
  Inbetriebnahmemonat (Monat zählt voll), Default-ND 20 Jahre (BMF-AfA-
  Tabelle für PV). GWG-Sofortabzug §6 Abs. 2 EStG: volle Basis als
  Aufwand im Inbetriebnahmejahr, danach 0 — relevant für Zubehör netto ≤ 800 €.
- **Sonder-AfA §7g Abs. 5 EStG** (`assets.sonderabschreibung_prozent`,
  0–50 %): einmaliger Zusatz-Aufwand im Inbetriebnahmejahr, getrennt im
  EÜR-Report ausgewiesen (`ausgaben_sonder_afa`).
- **Verkauf** (`assets.verkauft_am`, `verkaufserloes_netto`,
  `verkaufserloes_ust`): lineare AfA stoppt im Vormonat des Verkaufs.
  EÜR-Report enthält `einnahmen_veraeusserung_netto` (Erlös) und
  `ausgaben_restbuchwert_abgang` (Restbuchwert), Differenz = Veräußerungs-
  gewinn/-verlust. UStVA addiert `verkaufserloes_ust` zu `ust_einnahmen`.

### Dashboard
`get_dashboard()` liefert zusätzlich `einsparung_jahr` (Σ Eigenverbrauch
im Jahr × `strom_bezugspreis`) und `betreiber_modus` (taggenau aktuell).
Die UI zeigt im Privat-Modus die Ersparnis-Karte statt der Einnahmen-
Karte; im Gewerbe-Modus stehen beide nebeneinander, sofern Ersparnis > 0.

### Export & Backup
Vier Tauri-Commands für lokale Exporte (Datei-Schreiben direkt via
`std::fs` in Rust, Pfad kommt aus dem JS-Save/Open-Dialog):
- `export_buchungen_csv(path, jahr)` — semikolon-getrennt mit UTF-8-BOM,
  Beträge mit Komma-Dezimaltrenner. Buttons in `/euer` und `/anlage`.
- `export_anlagen_csv(path)` — Anlagenverzeichnis mit AfA pro Jahr.
- `export_backup(path)` / `import_backup(path)` — vollständiger JSON-Dump
  aller Tabellen mit `version`-Feld. Restore läuft in einer Transaktion
  und überschreibt **alle** bestehenden Daten (UI fragt explizit nach).

ELSTER-Übertragung ist bewusst nicht implementiert — braucht ERiC-Binary
+ Steuersignatur. UStVA-Werte werden gedruckt und manuell via
Mein ELSTER übertragen.

### Print-Layout
Druckbares A4-Layout via `window.print()` + `@media print` in
`src/app.css`. Markup-Konvention:
- `data-print="hide"` — versteckt beim Drucken (Filter, Buttons, Nav).
- `data-print="show"` — nur beim Drucken sichtbar (Report-Header mit
  Zeitraum + Stand-Datum).

Aktiv auf `/euer` und `/ust`.

### Hersteller-API
Stub-Command `import_from_vendor(von, bis)`. Erwartet `anker_api_url` und Token
in den Einstellungen. Aktuell wirft er einen klaren Fehler — der konkrete HTTP-
Adapter (Anker SOLIX / Fronius Solar.Web / SMA Sunny Portal) folgt sobald die
API-Spezifikation feststeht. Bis dahin Tageswerte manuell erfassen.

### Pages (`src/routes/`)
- `/` Dashboard — Tageswert, 7d / Monat / Jahr / Max-Tag, Jahres-Einnahmen
- `/erfassung` — Tageserfassung mit Plausibilitätsprüfung
- `/auszahlungen` — Bayernwerk-Gutschriften
- `/ausgaben` — Betriebsausgaben
- `/anlage` — Anlagenverzeichnis mit AfA-Berechnung
- `/euer` — Einnahmen-Überschuss-Rechnung pro Jahr
- `/ust` — UStVA-Berechnung pro Monat oder Jahr
- `/statistik` — Aggregate (Tag/Monat/Jahr/Max), Eigenverbrauchsquote
- `/einstellungen` — USt-Modus-Verlauf, Steuersatz, EV-Preis, API-Zugang

## Konventionen
- Keine SSR — alle Routen `ssr = false` via `+layout.ts`.
- Svelte 5 runes only: `$state`, `$derived`, `$effect`, `$props`. Kein `$:`.
- Komponenten-Imports aus `$lib/components/ui` (relative `..`-Pfade vermeiden).
- Beträge in `formatEUR`, kWh in `formatKWh`, Daten in `formatDateDE` (siehe
  `src/lib/utils.ts`).
- Vor Commits `bun run check` ausführen.
