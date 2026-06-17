#!/usr/bin/env python3
"""Anker-Cloud Tagesdaten-Sidecar fuer den Photovoltaik-Manager.

Liest Login-Daten aus ENV (ANKER_EMAIL, ANKER_PASSWORD, ANKER_COUNTRY),
holt fuer den Zeitraum [--von, --bis] (ISO YYYY-MM-DD) die Energie-Tageswerte
ueber die inoffizielle Anker-Solix-Cloud-API und streamed sie als NDJSON
auf stdout. Fehler gehen auf stderr und exit code != 0.

Streaming-Protokoll (eine JSON-Zeile pro Event auf stdout):
    {"kind": "site", "site_id": "abc-123"}
    {"kind": "row", "date": "2026-06-01", "erzeugung_kwh": 24.5, ...}
    {"kind": "skip", "date": "2026-06-02", "reason": "keine Solar-Daten"}
    ...
    {"kind": "summary", "warnings": [...]}

Rust liest stdout zeilenweise und committed alle N Zeilen (BATCH_SIZE=30)
in einer DB-Transaktion. Vorteil: bei Crash mid-stream sind die bisher
geholten Tage persistiert; User kann den Import resumen.

NDJSON-Progress auf stderr ist unabhaengig — wird als Tauri-Event an die UI
gepusht ohne die DB anzufassen.

Performance: 2 Anker-Calls × ca. 0.4s sleep + ca. 0.2s HTTP = ~1s/Tag.
Ein Jahr ~6 Min.

Abhaengigkeit: `anker-solix-api` direkt aus GitHub (v3.6.3), siehe
requirements.txt. Auf PyPI nicht veroeffentlicht.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
import sys
from datetime import date, datetime, timedelta
from typing import Any

try:
    from aiohttp import ClientSession
    # v3.6.3 hat noch die flache Modul-Struktur unter `api/`; die `main`-Branch
    # ist auf `anker_solix_api/` umgezogen, ist aber nicht released.
    from api.api import AnkerSolixApi
except ImportError as exc:
    print(
        json.dumps(
            {
                "error": (
                    f"Python-Abhaengigkeit fehlt: {exc}. "
                    "Bitte `pip install -r vendor-import-anker/requirements.txt` "
                    "im venv ausfuehren oder den Sidecar via "
                    "`./vendor-import-anker/build-sidecar.sh` bauen."
                )
            }
        ),
        file=sys.stderr,
    )
    sys.exit(2)


# Pause zwischen API-Calls — Anker hat Rate-Limits, die Community empfiehlt
# bei sequentiellen Calls 0.4s konservativ.
_CALL_DELAY_S = 0.4


def _parse_iso(s: str) -> date:
    return datetime.strptime(s, "%Y-%m-%d").date()


def _to_dt(d: date) -> datetime:
    """Anker-Lib erwartet `datetime`, nicht `date`. Mitternacht reicht."""
    return datetime(d.year, d.month, d.day)


def _daterange(von: date, bis: date) -> list[date]:
    if bis < von:
        raise ValueError("--bis liegt vor --von")
    days = (bis - von).days + 1
    return [von + timedelta(days=i) for i in range(days)]


def _kwh(value: Any, unit: str | None = None) -> float:
    """Anker liefert Strings/Floats fuer Energie. Wenn die Quelle in Wh ist,
    durch 1000 teilen — sonst direkt als kWh interpretieren.
    """
    if value is None or value == "":
        return 0.0
    try:
        v = abs(float(value))  # Anker liefert grid-export teils negativ
    except (TypeError, ValueError):
        return 0.0
    if (unit or "").lower() == "wh":
        v /= 1000.0
    return v


def _first_power_value(payload: dict, hardcoded_kwh: bool = True) -> float:
    """Liest den Tageswert aus power[0].value. Die thomluther-Lib (siehe
    energy_daily) liest hier IMMER und hardcoded `unit="kwh"`, weil der
    `power_unit`-Header der Anker-API zwar oft "wh" sagt, die Werte aber
    in kWh kommen. Wir machen es genauso.
    """
    items = payload.get("power") or []
    if not items:
        return 0.0
    first = items[0] if isinstance(items[0], dict) else {}
    val = first.get("value")
    return _kwh(val, "kwh" if hardcoded_kwh else payload.get("power_unit"))


def _first_charge_trend_value(payload: dict) -> float:
    """Wie _first_power_value, aber fuer das `charge_trend[]`-Array. Bei
    devType="grid" liegt der grid_to_home-Tageswert dort.
    """
    items = payload.get("charge_trend") or []
    if not items:
        return 0.0
    first = items[0] if isinstance(items[0], dict) else {}
    val = first.get("value")
    return _kwh(val, payload.get("charge_unit") or "kwh")


def _pick_first_site(sites: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    if not sites:
        raise RuntimeError(
            "Keine Anlage im Anker-Account gefunden. Pruefe Login-Daten und "
            "ob die Solarbank in der Anker-App eingerichtet ist."
        )
    site_id = next(iter(sites.keys()))
    return site_id, sites[site_id]


def _progress(msg: str, done: int = 0, total: int = 0) -> None:
    """Progress-Event auf stderr (eine NDJSON-Zeile pro Event). Rust liest
    den Stream zeilenweise mit und emittiert pro Zeile ein Tauri-Event
    `anker-import-progress` an die UI.
    """
    print(
        json.dumps({"progress": msg, "done": done, "total": total}),
        file=sys.stderr,
        flush=True,
    )


def _emit(payload: dict) -> None:
    """Eine NDJSON-Zeile auf stdout, sofort geflusht (Rust streamed mit)."""
    print(json.dumps(payload, ensure_ascii=False), flush=True)


async def _energy_for_day(
    api: AnkerSolixApi,
    site_id: str,
    day: date,
    dev_type: str,
) -> dict[str, Any]:
    """Eine `energy_analysis`-Antwort fuer einen einzelnen Tag.

    WICHTIG: `rangeType="week"` — nicht "day"! Mit type="day" liefert Anker
    Intra-Day-Werte (stuendlich), und die `*_total`-Felder kommen leer
    zurueck. Die thomluther-Lib (`energy_daily`) nutzt fuer single-day-
    Queries ebenfalls rangeType="week" — das ist die einzige Variante, bei
    der die API verlaesslich Tages-Totals in `solar_total`,
    `solar_to_grid_total`, `solar_to_home_total`, `grid_to_home_total`
    liefert.
    """
    dt = _to_dt(day)
    payload = await api.energy_analysis(
        siteId=site_id,
        deviceSn="",
        rangeType="week",
        startDay=dt,
        endDay=dt,
        devType=dev_type,
    )
    return payload if isinstance(payload, dict) else {}


async def _collect(
    email: str,
    password: str,
    country: str,
    von: date,
    bis: date,
) -> None:
    """Streamed pro Tag eine NDJSON-Zeile auf stdout. Kein Return-Wert —
    Rust liest den Stream und committed in Batches.
    """
    warnings: list[str] = []
    days = _daterange(von, bis)
    total_calls = len(days) * 2
    done_calls = 0
    imported_count = 0

    async with ClientSession() as session:
        api = AnkerSolixApi(email, password, country, session, logging.getLogger("anker"))
        _progress(f"Login Anker-Cloud ({email[:3]}***)", 0, total_calls)
        await api.update_sites()
        site_id, _site = _pick_first_site(api.sites)
        _emit({"kind": "site", "site_id": site_id})
        _progress(
            f"Anlage gefunden — {len(days)} Tage ({total_calls} Calls)",
            0,
            total_calls,
        )

        today = date.today()
        for idx, d in enumerate(days, start=1):
            iso = d.isoformat()

            # Zukunfts-Tage gar nicht erst abfragen — Anker antwortet
            # langsam mit leeren Daten und der User wuerde sich fragen
            # warum bei Monat=Juli minutenlang nichts passiert.
            if d > today:
                done_calls += 2
                _emit({"kind": "skip", "date": iso, "reason": "in der Zukunft"})
                warnings.append(f"{iso}: in der Zukunft — kein API-Call.")
                _progress(
                    f"Tag {idx}/{len(days)} ({iso}): Zukunft, uebersprungen",
                    done_calls,
                    total_calls,
                )
                continue

            try:
                sol = await _energy_for_day(api, site_id, d, "solar_production")
            except Exception as exc:  # noqa: BLE001
                warnings.append(
                    f"{iso}: solar_production fehlgeschlagen "
                    f"({type(exc).__name__}: {exc})"
                )
                sol = {}
            done_calls += 1
            await asyncio.sleep(_CALL_DELAY_S)

            try:
                grid = await _energy_for_day(api, site_id, d, "grid")
            except Exception as exc:  # noqa: BLE001
                warnings.append(
                    f"{iso}: grid fehlgeschlagen "
                    f"({type(exc).__name__}: {exc})"
                )
                grid = {}
            done_calls += 1
            await asyncio.sleep(_CALL_DELAY_S)

            # Tageswerte primaer aus power[]/charge_trend[] lesen (so wie es
            # die thomluther-Lib macht). Fallback auf *_total-Felder, die
            # bei single-day-Queries manchmal leer oder mit falschem Unit
            # zurueckkommen.
            sol_t_unit = sol.get("total_energy_unit")
            grid_t_unit = grid.get("total_energy_unit")

            erz = _first_power_value(sol) or _kwh(
                sol.get("solar_total"), sol_t_unit
            )
            ein = _first_power_value(grid) or _kwh(
                grid.get("solar_to_grid_total") or sol.get("solar_to_grid_total"),
                grid_t_unit or sol_t_unit,
            )
            net = _first_charge_trend_value(grid) or _kwh(
                grid.get("grid_to_home_total"), grid_t_unit
            )
            ev_from_api = _kwh(sol.get("solar_to_home_total"), sol_t_unit)

            # erzeugung = 0 heisst: Anker hat keinen Solar-Datensatz fuer
            # diesen Tag geliefert. Skip-Zeile emittieren — Rust schreibt
            # NICHT in die DB.
            if erz <= 0:
                _emit({"kind": "skip", "date": iso, "reason": "keine Solar-Daten (erz=0)"})
                warnings.append(
                    f"{iso}: keine Solar-Daten von Anker (erzeugung=0) — "
                    "Tag uebersprungen, bestehende Werte bleiben unveraendert."
                )
                _progress(
                    f"Tag {idx}/{len(days)} ({iso}): keine Daten",
                    done_calls,
                    total_calls,
                )
                continue

            ev = ev_from_api if ev_from_api > 0 else max(0.0, erz - ein)

            # Speicher-Flow (speicher_laden_kwh / speicher_entladen_kwh) wird
            # bewusst NICHT automatisch emittiert. Erste Implementierung las
            # `charge_total`/`discharge_total` aus der solar_production-
            # Antwort — die Werte enthalten aber Charge aus ALLEN Quellen
            # (inkl. Grid-Charging, 3rd-Party-PV), nicht nur Solar->Akku.
            # User hat 13.6 angezeigt bekommen statt 10.9 wie in der App.
            # TODO: separater devType="solarbank"-Call + subtrahieren von
            # grid_to_battery_total + third_party_pv_to_bat.
            _emit(
                {
                    "kind": "row",
                    "date": iso,
                    "erzeugung_kwh": round(erz, 3),
                    "eigenverbrauch_kwh": round(ev, 3),
                    "einspeisung_kwh": round(ein, 3),
                    "netzbezug_kwh": round(net, 3),
                }
            )
            imported_count += 1
            _progress(
                f"Tag {idx}/{len(days)} ({iso}): {erz:.1f} kWh",
                done_calls,
                total_calls,
            )

        _emit({"kind": "summary", "warnings": warnings})
        _progress(
            f"Fertig: {imported_count} Tage", total_calls, total_calls
        )


def main() -> int:
    parser = argparse.ArgumentParser(description="Anker-Cloud Tagesdaten-Import")
    parser.add_argument("--von", required=True, help="Startdatum ISO YYYY-MM-DD")
    parser.add_argument("--bis", required=True, help="Enddatum ISO YYYY-MM-DD")
    args = parser.parse_args()

    email = os.environ.get("ANKER_EMAIL", "").strip()
    password = os.environ.get("ANKER_PASSWORD", "").strip()
    country = os.environ.get("ANKER_COUNTRY", "DE").strip() or "DE"

    if not email or not password:
        print(
            json.dumps(
                {
                    "error": "ANKER_EMAIL und ANKER_PASSWORD muessen gesetzt sein."
                }
            ),
            file=sys.stderr,
        )
        return 2

    try:
        von = _parse_iso(args.von)
        bis = _parse_iso(args.bis)
    except ValueError as exc:
        print(json.dumps({"error": f"Ungueltiges Datum: {exc}"}), file=sys.stderr)
        return 2

    try:
        asyncio.run(_collect(email, password, country, von, bis))
    except Exception as exc:  # noqa: BLE001 — Sidecar muss alle Fehler serialisieren
        print(json.dumps({"error": f"{type(exc).__name__}: {exc}"}), file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
