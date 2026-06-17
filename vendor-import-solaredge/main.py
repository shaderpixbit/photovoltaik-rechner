#!/usr/bin/env python3
"""SolarEdge mySolarEdge Tagesdaten-Sidecar fuer den Photovoltaik-Manager.

Liest Login-Daten aus ENV (SOLAREDGE_API_KEY, SOLAREDGE_SITE_ID), holt
fuer den Zeitraum [--von, --bis] die Tageswerte ueber die offizielle
monitoringapi.solaredge.com und schreibt JSON nach stdout. Fehler nach
stderr + exit code != 0.

Vorteile vs. Anker:
- Offizielle, dokumentierte REST-API
- API-Key statt Account-Login (kein Session-Konflikt)
- 2 Calls fuer den ganzen Range (energy + energyDetails) — kein per-Tag
- Stdlib reicht: kein venv noetig

Endpoints (siehe SolarEdge Monitoring API Dok):
- /site/{id}/energy?startDate&endDate&timeUnit=DAY
    -> { energy: { values: [{date, value}], unit } }
    Tagesproduktion (Erzeugung).
- /site/{id}/energyDetails?startTime&endTime&timeUnit=DAY
        &meters=PRODUCTION,FEEDIN,PURCHASED,SELFCONSUMPTION
    -> { energyDetails: { meters: [{ type, values: [{date, value}] }] } }
    Eigenverbrauch / Einspeisung / Netzbezug — nur verfuegbar wenn die
    Anlage einen Smart Meter (oder Modbus-Energiezaehler) gemeldet hat.

Rate-Limit: 300 Requests / Tag / API-Key (lt. SolarEdge Doku). Wir
brauchen 2 Calls pro Import, also kein Problem.

Streaming-Protokoll (eine JSON-Zeile pro Event auf stdout — identisch zum
anker-Sidecar, damit Rust mit beiden gleich umgeht):
    {"kind": "site", "site_id": "..."}
    {"kind": "row", "date": "...", "erzeugung_kwh": ..., ...}
    {"kind": "skip", "date": "...", "reason": "..."}
    {"kind": "summary", "warnings": [...]}
NDJSON-Progress auf stderr (Rust forwarded als Tauri-Event).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import date, datetime
from typing import Any


_API_BASE = "https://monitoringapi.solaredge.com"
_TIMEOUT_S = 30


def _parse_iso(s: str) -> date:
    return datetime.strptime(s, "%Y-%m-%d").date()


def _progress(msg: str, done: int = 0, total: int = 0) -> None:
    print(
        json.dumps({"progress": msg, "done": done, "total": total}),
        file=sys.stderr,
        flush=True,
    )


def _emit(payload: dict) -> None:
    print(json.dumps(payload, ensure_ascii=False), flush=True)


def _get_json(url: str) -> dict[str, Any]:
    req = urllib.request.Request(url, headers={"User-Agent": "photovoltaik-pv-manager/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=_TIMEOUT_S) as resp:
            body = resp.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        # SolarEdge gibt Fehler-Detail oft im Body zurueck (JSON oder Plain).
        detail = ""
        try:
            detail = exc.read().decode("utf-8", errors="replace")[:300]
        except Exception:  # noqa: BLE001
            pass
        raise RuntimeError(f"HTTP {exc.code} {exc.reason}: {detail}") from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"Netzwerk-Fehler: {exc.reason}") from exc
    try:
        return json.loads(body)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"Antwort kein JSON: {body[:200]}") from exc


def _wh_to_kwh(value: Any, unit: str | None) -> float:
    """SolarEdge liefert energy in Wh (oder Wh nach unit-Feld). Normieren auf kWh."""
    if value is None or value == "":
        return 0.0
    try:
        v = float(value)
    except (TypeError, ValueError):
        return 0.0
    u = (unit or "").lower()
    if u == "wh":
        return v / 1000.0
    if u == "kwh":
        return v
    # Default: Wh annehmen (SolarEdge-Default fuer DAY-Granularitaet).
    return v / 1000.0


def _index_values(values: list[Any], unit: str | None) -> dict[str, float]:
    """[{date: "YYYY-MM-DD HH:MM:SS", value: X}] -> { "YYYY-MM-DD": X kWh }"""
    out: dict[str, float] = {}
    for item in values or []:
        if not isinstance(item, dict):
            continue
        d = item.get("date")
        if not d:
            continue
        day = str(d)[:10]
        out[day] = _wh_to_kwh(item.get("value"), unit)
    return out


def _collect(api_key: str, site_id: str, von: date, bis: date) -> None:
    """Streamed NDJSON auf stdout. SolarEdge holt alle Daten in 2 Calls,
    Rows werden danach in einem Burst emittiert — Rust batched sie genauso
    wie beim Anker-Streaming.
    """
    warnings: list[str] = []
    start_time = von.strftime("%Y-%m-%d") + " 00:00:00"
    end_time = bis.strftime("%Y-%m-%d") + " 23:59:59"

    _emit({"kind": "site", "site_id": site_id})

    _progress("SolarEdge: Erzeugung abrufen", 0, 2)
    energy_url = (
        f"{_API_BASE}/site/{site_id}/energy?"
        + urllib.parse.urlencode(
            {
                "startDate": von.isoformat(),
                "endDate": bis.isoformat(),
                "timeUnit": "DAY",
                "api_key": api_key,
            }
        )
    )
    energy = _get_json(energy_url).get("energy") or {}
    erz_by_day = _index_values(energy.get("values") or [], energy.get("unit"))

    _progress("SolarEdge: Eigenverbrauch / Einspeisung / Netzbezug", 1, 2)
    details_url = (
        f"{_API_BASE}/site/{site_id}/energyDetails?"
        + urllib.parse.urlencode(
            {
                "startTime": start_time,
                "endTime": end_time,
                "timeUnit": "DAY",
                "meters": "PRODUCTION,FEEDIN,PURCHASED,SELFCONSUMPTION",
                "api_key": api_key,
            }
        )
    )
    try:
        details = _get_json(details_url).get("energyDetails") or {}
    except RuntimeError as exc:
        warnings.append(
            f"energyDetails nicht verfuegbar ({exc}). Eigenverbrauch / "
            "Einspeisung / Netzbezug bleiben 0 — vermutlich kein Smart Meter."
        )
        details = {}

    detail_unit = details.get("unit")
    ein_by_day: dict[str, float] = {}
    net_by_day: dict[str, float] = {}
    selfcons_by_day: dict[str, float] = {}
    for meter in details.get("meters") or []:
        if not isinstance(meter, dict):
            continue
        m_type = (meter.get("type") or "").upper()
        idx = _index_values(meter.get("values") or [], detail_unit)
        if m_type == "FEEDIN":
            ein_by_day = idx
        elif m_type == "PURCHASED":
            net_by_day = idx
        elif m_type == "SELFCONSUMPTION":
            selfcons_by_day = idx

    imported = 0
    for d_str in sorted(erz_by_day.keys()):
        if d_str < von.isoformat() or d_str > bis.isoformat():
            continue
        erz = erz_by_day.get(d_str, 0.0)
        if erz <= 0:
            _emit({"kind": "skip", "date": d_str, "reason": "keine Solar-Daten (erz=0)"})
            warnings.append(
                f"{d_str}: keine Solar-Daten — Tag uebersprungen, "
                "bestehende Werte bleiben unveraendert."
            )
            continue
        ein = ein_by_day.get(d_str, 0.0)
        net = net_by_day.get(d_str, 0.0)
        ev = selfcons_by_day.get(d_str)
        if not ev or ev <= 0:
            ev = max(0.0, erz - ein)
        _emit(
            {
                "kind": "row",
                "date": d_str,
                "erzeugung_kwh": round(erz, 3),
                "eigenverbrauch_kwh": round(ev, 3),
                "einspeisung_kwh": round(ein, 3),
                "netzbezug_kwh": round(net, 3),
            }
        )
        imported += 1

    _emit({"kind": "summary", "warnings": warnings})
    _progress(f"Fertig: {imported} Tage", 2, 2)


def main() -> int:
    parser = argparse.ArgumentParser(description="SolarEdge Tagesdaten-Import")
    parser.add_argument("--von", required=True, help="Startdatum ISO YYYY-MM-DD")
    parser.add_argument("--bis", required=True, help="Enddatum ISO YYYY-MM-DD")
    args = parser.parse_args()

    api_key = os.environ.get("SOLAREDGE_API_KEY", "").strip()
    site_id = os.environ.get("SOLAREDGE_SITE_ID", "").strip()
    if not api_key or not site_id:
        print(
            json.dumps(
                {
                    "error": "SOLAREDGE_API_KEY und SOLAREDGE_SITE_ID muessen gesetzt sein."
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
    if bis < von:
        print(json.dumps({"error": "--bis liegt vor --von"}), file=sys.stderr)
        return 2

    try:
        _collect(api_key, site_id, von, bis)
    except Exception as exc:  # noqa: BLE001
        print(json.dumps({"error": f"{type(exc).__name__}: {exc}"}), file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
