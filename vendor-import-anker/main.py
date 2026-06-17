#!/usr/bin/env python3
"""Anker-Cloud Tagesdaten-Sidecar fuer den Photovoltaik-Manager.

Liest Login-Daten aus ENV (ANKER_EMAIL, ANKER_PASSWORD, ANKER_COUNTRY),
holt fuer den Zeitraum [--von, --bis] (ISO YYYY-MM-DD) die Energie-Tageswerte
ueber die inoffizielle Anker-Solix-Cloud-API und schreibt eine JSON-Antwort
auf stdout. Fehler gehen auf stderr und exit code != 0.

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

Abhaengigkeit: `anker-solix-api` direkt aus GitHub (v3.6.3+), siehe
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
    from anker_solix_api.api import AnkerSolixApi
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
# pro Account hoechstens alle paar Sekunden eine Anfrage. Bei 31 Tagen *
# 2 Calls = 62 Aufrufe sind 0.4s konservativ.
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


async def _energy_for_day(
    api: AnkerSolixApi,
    site_id: str,
    day: date,
    dev_type: str,
) -> dict[str, Any]:
    """Eine `energy_analysis`-Antwort fuer einen einzelnen Tag (Range = 1 Tag).
    Damit liefert die Anker-API alle `*_total`-Felder als Tageswerte.
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
    async with ClientSession() as session:
        api = AnkerSolixApi(email, password, country, session, logging.getLogger("anker"))

        # Sites laden — `update_sites` triggert auch das Login wenn noch nicht erfolgt.
        await api.update_sites()
        site_id, _site = _pick_first_site(api.sites)

        result_rows: list[dict[str, Any]] = []
        for d in _daterange(von, bis):
            iso = d.isoformat()
            try:
                sol = await _energy_for_day(api, site_id, d, "solar_production")
                await asyncio.sleep(_CALL_DELAY_S)
                grid = await _energy_for_day(api, site_id, d, "grid")
                await asyncio.sleep(_CALL_DELAY_S)
            except Exception as exc:  # noqa: BLE001
                warnings.append(f"{iso}: API-Fehler {type(exc).__name__}: {exc}")
                continue

            sol_unit = sol.get("power_unit")
            grid_unit = grid.get("power_unit")

            erzeugung = _kwh(sol.get("solar_total"), sol_unit)
            einspeisung = _kwh(sol.get("solar_to_grid_total"), sol_unit)
            eigenverbrauch = _kwh(sol.get("solar_to_home_total"), sol_unit)
            netzbezug = _kwh(grid.get("grid_to_home_total"), grid_unit)

            # Manche Solarbank-Modelle geben `solar_to_home_total` leer zurueck.
            # Fallback: Erzeugung - Einspeisung (vorausgesetzt kein Batterie-
            # Eigenverbrauch, der separat ueber `battery_to_home` laeuft).
            if eigenverbrauch <= 0 and erzeugung > 0:
                eigenverbrauch = max(0.0, erzeugung - einspeisung)

            if erzeugung == 0 and einspeisung == 0 and netzbezug == 0:
                warnings.append(f"{iso}: alle Werte 0 — vermutlich kein Datensatz.")
                continue

            result_rows.append(
                {
                    "date": iso,
                    "erzeugung_kwh": round(erzeugung, 3),
                    "eigenverbrauch_kwh": round(eigenverbrauch, 3),
                    "einspeisung_kwh": round(einspeisung, 3),
                    "netzbezug_kwh": round(netzbezug, 3),
                }
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
