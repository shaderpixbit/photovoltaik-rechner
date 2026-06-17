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

Abhaengigkeit: `anker-solix-api` (PyPI, von thomluther). Wird vom Sidecar-Build
via requirements.txt installiert.
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

# anker-solix-api ist eine inoffizielle Anker-Cloud-Bibliothek.
# Wenn das Paket nicht installiert ist (z.B. Dev ohne pip install),
# soll der Fehler sauber an die App weitergereicht werden.
try:
    from aiohttp import ClientSession
    from api.api import AnkerSolixApi  # type: ignore
except ImportError as exc:
    print(
        json.dumps(
            {
                "error": f"Python-Abhaengigkeit fehlt: {exc}. "
                "Bitte `pip install -r vendor-import-anker/requirements.txt` "
                "ausfuehren oder den Sidecar via PyInstaller bauen."
            }
        ),
        file=sys.stderr,
    )
    sys.exit(2)


def _parse_iso(s: str) -> date:
    return datetime.strptime(s, "%Y-%m-%d").date()


def _daterange(von: date, bis: date) -> list[date]:
    if bis < von:
        raise ValueError("--bis liegt vor --von")
    days = (bis - von).days + 1
    return [von + timedelta(days=i) for i in range(days)]


def _kwh(value: Any) -> float:
    """Anker liefert Strings/Floats fuer kWh. Robust nach float konvertieren."""
    if value is None or value == "":
        return 0.0
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def _pick_first_site(sites: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    """Erste Anlage des Accounts. Multi-Site-Setups werden hier nicht unterstuetzt
    — dann muesste das Backend einen `--site-id` Parameter durchreichen.
    """
    if not sites:
        raise RuntimeError(
            "Keine Anlage im Anker-Account gefunden. Pruefe Login-Daten und "
            "ob die Solarbank in der Anker-App eingerichtet ist."
        )
    site_id = next(iter(sites.keys()))
    return site_id, sites[site_id]


def _extract_day_rows(payload: dict[str, Any]) -> dict[str, dict[str, float]]:
    """Aus der Anker-energy_analysis-Antwort die Tagessummen extrahieren.

    Das Schema variiert zwischen Solarbank-Modellen und API-Versionen. Wir
    schauen in mehreren bekannten Pfaden nach und mappen lose:

    - `power_site_list` oder `energy_list` ist die uebliche Container-Liste.
    - Pro Eintrag erwarten wir `date` (YYYY-MM-DD) und Felder wie
      `solar_generation` / `home_usage` / `grid_to_home` / `solar_to_grid`.
    """
    rows: dict[str, dict[str, float]] = {}

    candidates = (
        payload.get("power_site_list")
        or payload.get("energy_list")
        or payload.get("data", {}).get("power_site_list")
        or payload.get("data", {}).get("energy_list")
        or []
    )
    if isinstance(candidates, dict):
        # Manche Versionen liefern ein Dict mit Datum als Key.
        items = [{"date": k, **(v if isinstance(v, dict) else {})} for k, v in candidates.items()]
    else:
        items = candidates

    for item in items:
        if not isinstance(item, dict):
            continue
        d = item.get("date") or item.get("day") or item.get("time")
        if not d:
            continue
        # YYYY-MM-DD aus z.B. "2026-06-01 00:00:00" extrahieren.
        d = str(d)[:10]
        erzeugung = _kwh(item.get("solar_generation") or item.get("solar") or item.get("solar_kwh"))
        eigenverbrauch = _kwh(
            item.get("home_usage") or item.get("home") or item.get("home_kwh")
        )
        einspeisung = _kwh(
            item.get("solar_to_grid") or item.get("to_grid") or item.get("grid_export")
        )
        netzbezug = _kwh(
            item.get("grid_to_home") or item.get("from_grid") or item.get("grid_import")
        )
        # Eigenverbrauch ist fuer die App = was zuhause verbraucht wurde aus PV.
        # Anker liefert oft "home_usage" inkl. Netzbezug — wir subtrahieren.
        if eigenverbrauch > 0 and netzbezug > 0:
            eigenverbrauch = max(0.0, eigenverbrauch - netzbezug)
        rows[d] = {
            "erzeugung_kwh": round(erzeugung, 3),
            "eigenverbrauch_kwh": round(eigenverbrauch, 3),
            "einspeisung_kwh": round(einspeisung, 3),
            "netzbezug_kwh": round(netzbezug, 3),
        }
    return rows


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
        await api.update_sites()
        site_id, _site = _pick_first_site(api.sites)

        # energy_analysis nimmt rangeType "day" und liefert Tageswerte zwischen
        # startDay/endDay. devType "solar" deckt PV + Solarbank ab.
        payload = await api.energy_analysis(
            siteId=site_id,
            deviceSn="",
            rangeType="day",
            startDay=von,
            endDay=bis,
            devType="solar",
        )
        rows_by_date = _extract_day_rows(payload if isinstance(payload, dict) else {})

        result_rows = []
        for d in _daterange(von, bis):
            iso = d.isoformat()
            r = rows_by_date.get(iso)
            if r is None:
                warnings.append(f"Kein Datensatz fuer {iso} in der API-Antwort.")
                continue
            result_rows.append({"date": iso, **r})

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
