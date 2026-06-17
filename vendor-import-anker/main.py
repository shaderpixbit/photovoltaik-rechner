#!/usr/bin/env python3
"""Anker-Cloud Tagesdaten-Sidecar fuer den Photovoltaik-Manager.

Liest Login-Daten aus ENV (ANKER_EMAIL, ANKER_PASSWORD, ANKER_COUNTRY),
holt fuer den Zeitraum [--von, --bis] (ISO YYYY-MM-DD) die Energie-Tageswerte
ueber die inoffizielle Anker-Solix-Cloud-API und schreibt eine JSON-Antwort
auf stdout. Fehler gehen auf stderr und exit code != 0.

Strategie: pro Tag zwei `energy_analysis(rangeType="day", startDay=endDay=tag)`
Calls — einer mit `devType="solar_production"` fuer Erzeugung/Einspeisung,
einer mit `devType="grid"` fuer Netzbezug. Anker liefert nur bei
single-day-Range alle `*_total`-Felder als Tageswerte (Multi-Day-Ranges
geben aggregierte Summen, nicht Tagessummen).

Performance: 2 Calls × ca. 0.4s sleep + ca. 0.2s HTTP = ~1s/Tag. Ein Jahr
~6 Min. Wer schneller will: `device_pv_energy_daily` der Lib probieren,
oder parallel mit asyncio.Semaphore — beides nicht-trivial wegen
Rate-Limits.

NDJSON-Progress auf stderr nach jedem Tag, Rust forwarded als
`anker-import-progress`-Tauri-Event an die UI.

Ausgabe-Schema (stdout, eine Zeile JSON):
    {
      "rows": [
        { "date": "2026-06-01",
          "erzeugung_kwh": 24.5,
          "eigenverbrauch_kwh": 8.2,
          "einspeisung_kwh": 16.3,
          "netzbezug_kwh": 2.1 },
        ...
      ],
      "site_id": "abc-123",
      "warnings": []
    }

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


def _kwh(value: Any, power_unit: str | None = None) -> float:
    """Anker liefert Strings/Floats fuer Energie. Wenn die Quelle in Wh ist,
    durch 1000 teilen — sonst direkt als kWh interpretieren.
    """
    if value is None or value == "":
        return 0.0
    try:
        v = float(value)
    except (TypeError, ValueError):
        return 0.0
    if (power_unit or "").lower() == "wh":
        v /= 1000.0
    return v


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


async def _energy_for_day(
    api: AnkerSolixApi,
    site_id: str,
    day: date,
    dev_type: str,
) -> dict[str, Any]:
    """Eine `energy_analysis`-Antwort fuer einen einzelnen Tag (Range = 1 Tag).
    Bei single-day-Range liefert die Anker-API alle `*_total`-Felder als
    Tageswerte (statt aggregierter Range-Summen).
    """
    dt = _to_dt(day)
    payload = await api.energy_analysis(
        siteId=site_id,
        deviceSn="",
        rangeType="day",
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
) -> dict[str, Any]:
    warnings: list[str] = []
    days = _daterange(von, bis)
    total_calls = len(days) * 2
    done_calls = 0

    async with ClientSession() as session:
        api = AnkerSolixApi(email, password, country, session, logging.getLogger("anker"))
        _progress(f"Login Anker-Cloud ({email[:3]}***)", 0, total_calls)
        await api.update_sites()
        site_id, _site = _pick_first_site(api.sites)
        _progress(
            f"Anlage gefunden — {len(days)} Tage ({total_calls} Calls)",
            0,
            total_calls,
        )

        result_rows: list[dict[str, Any]] = []
        for idx, d in enumerate(days, start=1):
            iso = d.isoformat()

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

            sol_unit = sol.get("power_unit")
            grid_unit = grid.get("power_unit")

            erz = _kwh(sol.get("solar_total"), sol_unit)
            ein = _kwh(sol.get("solar_to_grid_total"), sol_unit)
            ev_from_api = _kwh(sol.get("solar_to_home_total"), sol_unit)
            net = _kwh(grid.get("grid_to_home_total"), grid_unit)

            # erzeugung = 0 heisst: Anker hat keinen Solar-Datensatz fuer
            # diesen Tag geliefert (Future-Tag, Lueckendaten, oder System-
            # Ausfall). Niemals einen Row mit erz=0 emittieren — sonst
            # ueberschreibt die UPSERT bestehende, manuell oder via frueheren
            # Import gepflegte Werte.
            if erz <= 0:
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

            # Eigenverbrauch primaer aus API, fallback Erzeugung - Einspeisung.
            ev = ev_from_api if ev_from_api > 0 else max(0.0, erz - ein)

            result_rows.append(
                {
                    "date": iso,
                    "erzeugung_kwh": round(erz, 3),
                    "eigenverbrauch_kwh": round(ev, 3),
                    "einspeisung_kwh": round(ein, 3),
                    "netzbezug_kwh": round(net, 3),
                }
            )
            _progress(
                f"Tag {idx}/{len(days)} ({iso}): {erz:.1f} kWh",
                done_calls,
                total_calls,
            )

        _progress(
            f"Fertig: {len(result_rows)} Tage importiert", total_calls, total_calls
        )
        return {
            "rows": result_rows,
            "site_id": site_id,
            "warnings": warnings,
        }


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
        result = asyncio.run(_collect(email, password, country, von, bis))
    except Exception as exc:  # noqa: BLE001 — Sidecar muss alle Fehler serialisieren
        print(json.dumps({"error": f"{type(exc).__name__}: {exc}"}), file=sys.stderr)
        return 1

    json.dump(result, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
