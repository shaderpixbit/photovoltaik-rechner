#!/usr/bin/env python3
"""Anker-Cloud Tagesdaten-Sidecar fuer den Photovoltaik-Manager.

Liest Login-Daten aus ENV (ANKER_EMAIL, ANKER_PASSWORD, ANKER_COUNTRY),
holt fuer den Zeitraum [--von, --bis] (ISO YYYY-MM-DD) die Energie-Tageswerte
ueber die inoffizielle Anker-Solix-Cloud-API und schreibt eine JSON-Antwort
auf stdout. Fehler gehen auf stderr und exit code != 0.

Performance: chunked nach Wochen-Bloecken (max. 7 Tage pro API-Call). Das
liefert die Tageswerte fuer eine ganze Woche in einem einzigen
`energy_analysis(rangeType="week")`-Call. Pro Wochen-Chunk werden 2 Calls
benoetigt (solar_production + grid). Fuer ein Jahr also ca. 106 Calls statt
730 — bei 0.4s sleep ca. 45s statt 5 Min.

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
# bei 7-Tages-Chunks 0.4s konservativ.
_CALL_DELAY_S = 0.4
# Anker akzeptiert mit rangeType="week" Range-Queries; 7 Tage pro Block ist
# das was die thomluther-Lib selbst nutzt (siehe energy_daily()).
_CHUNK_DAYS = 7


def _parse_iso(s: str) -> date:
    return datetime.strptime(s, "%Y-%m-%d").date()


def _to_dt(d: date) -> datetime:
    """Anker-Lib erwartet `datetime`, nicht `date`. Mitternacht reicht."""
    return datetime(d.year, d.month, d.day)


def _chunks(von: date, bis: date, step: int) -> list[tuple[date, date]]:
    if bis < von:
        raise ValueError("--bis liegt vor --von")
    out: list[tuple[date, date]] = []
    cur = von
    while cur <= bis:
        end = min(cur + timedelta(days=step - 1), bis)
        out.append((cur, end))
        cur = end + timedelta(days=1)
    return out


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


def _index_by_day(items: list[Any], unit: str | None) -> dict[str, float]:
    """power[] / charge_trend[] -> {"YYYY-MM-DD": kwh}.

    Anker liefert die Werte teils mit Minus-Vorzeichen (z.B. solar_to_grid im
    grid-Response). Wir nehmen den absoluten Betrag — die Richtung steckt im
    Feldnamen, nicht im Vorzeichen.
    """
    out: dict[str, float] = {}
    for item in items or []:
        if not isinstance(item, dict):
            continue
        t = item.get("time")
        if not t:
            continue
        day = str(t)[:10]
        out[day] = abs(_kwh(item.get("value"), unit))
    return out


def _progress(msg: str) -> None:
    """Best-effort Progress auf stderr (ein Zeile pro Event). Rust kann
    optional darauf hoeren und an die UI weiterreichen; failt es nicht,
    kostet es nur ein paar Bytes.
    """
    print(json.dumps({"progress": msg}), file=sys.stderr, flush=True)


async def _collect(
    email: str,
    password: str,
    country: str,
    von: date,
    bis: date,
) -> dict[str, Any]:
    warnings: list[str] = []
    chunks = _chunks(von, bis, _CHUNK_DAYS)
    total_calls = len(chunks) * 2
    done_calls = 0

    async with ClientSession() as session:
        api = AnkerSolixApi(email, password, country, session, logging.getLogger("anker"))
        _progress(f"Login Anker-Cloud ({email[:3]}***)")
        await api.update_sites()
        site_id, _site = _pick_first_site(api.sites)
        _progress(f"Anlage: {site_id} — {len(chunks)} Wochen-Chunks ({total_calls} Calls)")

        # Pro Chunk zwei Calls — solar_production + grid. Mit rangeType="week"
        # liefert Anker daily values im `power[]` Array fuer den ganzen Chunk.
        per_day_erz: dict[str, float] = {}
        per_day_ein: dict[str, float] = {}
        per_day_net: dict[str, float] = {}

        for idx, (c_von, c_bis) in enumerate(chunks, start=1):
            try:
                sol = await api.energy_analysis(
                    siteId=site_id,
                    deviceSn="",
                    rangeType="week",
                    startDay=_to_dt(c_von),
                    endDay=_to_dt(c_bis),
                    devType="solar_production",
                )
            except Exception as exc:  # noqa: BLE001
                warnings.append(
                    f"{c_von}..{c_bis}: solar_production fehlgeschlagen "
                    f"({type(exc).__name__}: {exc})"
                )
                sol = {}
            done_calls += 1
            _progress(f"Chunk {idx}/{len(chunks)}: solar_production OK ({done_calls}/{total_calls})")
            await asyncio.sleep(_CALL_DELAY_S)

            try:
                grid = await api.energy_analysis(
                    siteId=site_id,
                    deviceSn="",
                    rangeType="week",
                    startDay=_to_dt(c_von),
                    endDay=_to_dt(c_bis),
                    devType="grid",
                )
            except Exception as exc:  # noqa: BLE001
                warnings.append(
                    f"{c_von}..{c_bis}: grid fehlgeschlagen "
                    f"({type(exc).__name__}: {exc})"
                )
                grid = {}
            done_calls += 1
            _progress(f"Chunk {idx}/{len(chunks)}: grid OK ({done_calls}/{total_calls})")
            await asyncio.sleep(_CALL_DELAY_S)

            if isinstance(sol, dict):
                per_day_erz.update(
                    _index_by_day(sol.get("power") or [], sol.get("power_unit"))
                )
            if isinstance(grid, dict):
                # grid: power[] = daily solar_to_grid (negative), charge_trend[] = daily grid_to_home
                per_day_ein.update(
                    _index_by_day(grid.get("power") or [], grid.get("power_unit"))
                )
                per_day_net.update(
                    _index_by_day(
                        grid.get("charge_trend") or [], grid.get("charge_unit")
                    )
                )

        # Tabellarisch zusammenfuegen, eigenverbrauch berechnen.
        result_rows: list[dict[str, Any]] = []
        cur = von
        while cur <= bis:
            iso = cur.isoformat()
            erz = per_day_erz.get(iso, 0.0)
            ein = per_day_ein.get(iso, 0.0)
            net = per_day_net.get(iso, 0.0)
            if erz <= 0 and ein <= 0 and net <= 0:
                warnings.append(f"{iso}: alle Werte 0 — vermutlich kein Datensatz.")
                cur += timedelta(days=1)
                continue
            # Eigenverbrauch = Erzeugung - Einspeisung (auf Tages-Ebene fuer
            # Systeme ohne Batterie exakt; mit Batterie naeherungsweise korrekt,
            # weil sich Lade/Entlade-Zyklen innerhalb des Tages aufheben).
            ev = max(0.0, erz - ein)
            result_rows.append(
                {
                    "date": iso,
                    "erzeugung_kwh": round(erz, 3),
                    "eigenverbrauch_kwh": round(ev, 3),
                    "einspeisung_kwh": round(ein, 3),
                    "netzbezug_kwh": round(net, 3),
                }
            )
            cur += timedelta(days=1)

        _progress(f"Fertig: {len(result_rows)} Tage")
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
