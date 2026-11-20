#!/usr/bin/env python3
"""Rebuild supported_devices.txt from local recovered sources.

The name list and the decompiled tables are local review inputs. This script
does not open a device. Re-run it only on a checkout that still has those inputs.
The committed table is what tests load.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

SCHEMA = "2"
COLUMNS = (
    "name",
    "role",
    "usb_vid",
    "usb_pid",
    "l6_product",
    "engine_profile",
    "engine_operation",
    "capability_state",
    "source",
    "evidence",
    "owner_live_rig",
    "synthetic_fixture",
    "gap",
)
EMPTY = "-"
SOURCE_TOKENS = {
    "workbench-product-constants",
    "monkey-default-cfg",
    "committed-usb-record",
    "workbench-hd-static",
    "owner-name-list",
}

# USB ids copied from the committed Rust records. The inventory test checks
# the generated rows against those constants.
COMMITTED_USB = {
    "PODxt Live": (0x0E41, 0x4650),
    "Variax USB Interface": (0x0E41, 0x534D),
    "POD HD 500": (0x0E41, 0x414D),
    "POD HD": (0x0E41, 0x4156),
    "POD HD Pro": (0x0E41, 0x4157),
    "POD HD Pro X": (0x0E41, 0x415A),
    "POD HD500X": (0x0E41, 0x4159),
    "POD HD 300": (0x0E41, 0x5057),
    "POD HD 400": (0x0E41, 0x5058),
}
ENGINE = {
    "PODxt Live": ("PodxtLive", "EnumerateUsb", "Locked"),
    "Variax 300/600": ("Variax600", "EnumerateUsb", "Locked"),
    "POD HD500X": ("PodHd500x", "OpenPodPreset", "Unverified"),
    "JTV-69": ("Jtv69", "OpenGuitarPreset", "Unverified"),
    "Workbench HD": ("WorkbenchHd", "OpenGuitarPreset", "Unverified"),
}
OWNER_RIG = {"PODxt Live", "Variax 300/600"}
PARTIAL = {"PODxt Live", "Variax 300/600"}
FAMILY_NAMES = {"Tyler Variax"}
GUITAR_NAMES = {
    "Variax 300/600",
    "Variax 500/700",
    "Variax Acoustic",
    "Variax Bass",
    "Variax Bass 705",
}
BRIDGE_NAMES = {"PODxt Live", "Variax USB Interface"}
# Monkey's display name for 0x00070000. Workbench names that same id
# "Variax 500/700". The id stays on the Workbench row.
MONKEY_NAME_EXCEPTIONS = {0x00070000: "Variax"}


def hex_id(value: int, width: int) -> str:
    return f"0x{value:0{width}X}"


def parse_java(path: Path) -> tuple[dict[str, int], list[tuple[int, str]]]:
    text = path.read_text(encoding="utf-8")
    constants = {
        name: int(value)
        for name, value in re.findall(
            r"public static final int (\w+) = (\d+);", text
        )
    }
    method = text.split("GetNameForID", 1)[1].split("GetShortNameForID", 1)[0]
    named = [
        (int(value), label)
        for value, label in re.findall(r'case (\d+): \{\s*return "([^"]+)";', method)
    ]
    if not constants or not named:
        raise SystemExit(f"no product constants parsed from {path}")
    return constants, named


def parse_monkey(path: Path) -> list[dict[str, str | None]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    rows = []
    for item in payload["hardware"]:
        attributes = item["attributes"]
        rows.append(
            {
                "name": item["fields"]["name"],
                "usb_pid": attributes.get("usb_id"),
                "l6_product": attributes.get("midi_id"),
            }
        )
    return rows


def blank() -> dict[str, str]:
    return {column: EMPTY for column in COLUMNS}


def fill_engine(row: dict[str, str]) -> None:
    profile, operation, state = ENGINE.get(row["name"], ("none", "none", "none"))
    row["engine_profile"] = profile
    row["engine_operation"] = operation
    row["capability_state"] = state


def sourced_row(name: str, sources: set[str]) -> dict[str, str]:
    row = blank()
    row["name"] = name
    row["source"] = "+".join(sorted(sources))
    for token in sources:
        if token not in SOURCE_TOKENS:
            raise SystemExit(f"unknown source {token}")
    if name in BRIDGE_NAMES:
        row["role"] = "bridge"
    elif name in FAMILY_NAMES:
        row["role"] = "family"
    elif name in GUITAR_NAMES:
        row["role"] = "guitar"
    elif name == "Workbench HD":
        row["role"] = "family"
    elif name.startswith("JTV-"):
        row["role"] = "product"
    else:
        row["role"] = "product"
    if name in PARTIAL:
        row["evidence"] = "partial"
    elif name in FAMILY_NAMES:
        row["evidence"] = "family-only"
    elif name in {"JTV-59", "JTV-69", "JTV-89", "JTV-89F", "Workbench HD"}:
        row["evidence"] = "name-only"
    else:
        row["evidence"] = "sourced-id"
    row["owner_live_rig"] = "yes" if name in OWNER_RIG else "no"
    row["synthetic_fixture"] = (
        "in-memory-read-session" if name in PARTIAL else "missing"
    )
    fill_engine(row)
    gaps = []
    if name in PARTIAL:
        gaps.extend(
            ["read-requires-excluded-transfer", "partial-metadata", "not-a-backup"]
        )
    elif name == "Tyler Variax":
        gaps.extend(["family-only", "no-settings-codec"])
    elif name == "JTV-69":
        gaps.extend(["family-id-not-this-model", "no-settings-codec", "fixture-missing"])
    elif name == "Variax USB Interface":
        gaps.extend(["bridge-without-guitar", "no-settings-codec", "fixture-missing"])
    elif name in {"JTV-59", "JTV-89", "JTV-89F", "Workbench HD"}:
        gaps.extend(["name-only", "no-settings-codec", "fixture-missing"])
    else:
        gaps.extend(["no-settings-codec", "fixture-missing"])
    row["gap"] = ",".join(gaps)
    return row


def build(java_named: list[tuple[int, str]], java_constants: dict[str, int], monkey: list[dict[str, str | None]], names: list[str]) -> list[dict[str, str]]:
    by_id: dict[int, dict[str, str]] = {}
    for value, label in java_named:
        row = sourced_row(label, {"workbench-product-constants"})
        row["l6_product"] = hex_id(value, 8)
        by_id[value] = row
    bass = java_constants["VARIAX_BASS"]
    if bass in by_id:
        raise SystemExit("VARIAX_BASS unexpectedly has a Workbench display name")
    bass_row = sourced_row("Variax Bass", {"workbench-product-constants"})
    bass_row["l6_product"] = hex_id(bass, 8)
    by_id[bass] = bass_row

    for item in monkey:
        midi = int(item["l6_product"], 16) if item["l6_product"] else None
        usb_pid = int(item["usb_pid"], 16) if item["usb_pid"] else None
        if midi in MONKEY_NAME_EXCEPTIONS and item["name"] == MONKEY_NAME_EXCEPTIONS[midi]:
            continue
        if midi is not None and midi in by_id:
            row = by_id[midi]
            if row["name"] != item["name"]:
                raise SystemExit(
                    f"monkey name {item['name']!r} disagrees with {row['name']!r} for {midi:#x}"
                )
            row["source"] = "+".join(
                sorted(set(row["source"].split("+")) | {"monkey-default-cfg"})
            )
        elif midi is not None:
            row = sourced_row(item["name"], {"monkey-default-cfg"})
            row["l6_product"] = hex_id(midi, 8)
            by_id[midi] = row
        else:
            raise SystemExit(f"monkey row {item['name']} has no midi id")
        if usb_pid is not None:
            row["usb_pid"] = hex_id(usb_pid, 4)
            if "monkey-default-cfg" not in row["source"]:
                row["source"] = "+".join(
                    sorted(set(row["source"].split("+")) | {"monkey-default-cfg"})
                )

    extra = 0
    for name, (vid, pid) in COMMITTED_USB.items():
        matches = [row for row in by_id.values() if row["name"] == name]
        if matches:
            row = matches[0]
        else:
            extra -= 1
            row = sourced_row(name, {"committed-usb-record"})
            by_id[extra] = row
        row["usb_vid"] = hex_id(vid, 4)
        row["usb_pid"] = hex_id(pid, 4)
        row["source"] = "+".join(
            sorted(set(row["source"].split("+")) | {"committed-usb-record"})
        )

    for name in ("JTV-59", "JTV-69", "JTV-89", "JTV-89F", "Workbench HD"):
        if any(row["name"] == name for row in by_id.values()):
            raise SystemExit(f"{name} unexpectedly received a product id")
        extra -= 1
        by_id[extra] = sourced_row(name, {"workbench-hd-static"})

    for row in by_id.values():
        if row["name"] == "Tyler Variax":
            row["source"] = "+".join(
                sorted(set(row["source"].split("+")) | {"workbench-hd-static"})
            )
    rows = list(by_id.values())
    canonical = {row["name"] for row in rows}
    for name in names:
        if name in canonical:
            continue
        alias = blank()
        alias["name"] = name
        alias["role"] = "alias"
        alias["source"] = "owner-name-list"
        alias["evidence"] = "name-only"
        alias["owner_live_rig"] = "no"
        alias["synthetic_fixture"] = "missing"
        alias["gap"] = "alias-not-an-id"
        rows.append(alias)
    # The engine profile is labeled "Variax 600". The hyphen draft also split
    # the family ids into these four names. They stay aliases so none of them
    # receives 0x00070004 or another product id.
    present = {row["name"] for row in rows}
    for name in ("Variax 300", "Variax 500", "Variax 600", "Variax 700"):
        if name in present:
            continue
        alias = blank()
        alias["name"] = name
        alias["role"] = "alias"
        alias["source"] = "owner-name-list"
        alias["evidence"] = "name-only"
        alias["owner_live_rig"] = "no"
        alias["synthetic_fixture"] = "missing"
        alias["gap"] = "alias-not-an-id"
        rows.append(alias)
    rows.sort(key=lambda row: (row["name"].casefold(), row["l6_product"], row["usb_pid"]))
    return rows


def render(rows: list[dict[str, str]]) -> str:
    lines = [
        f"schema\t{SCHEMA}",
        "\t".join(COLUMNS),
    ]
    for row in rows:
        if set(row) != set(COLUMNS):
            raise SystemExit(f"bad row keys for {row.get('name')}")
        lines.append("\t".join(row[column] for column in COLUMNS))
    return "\n".join(lines) + "\n"


def hyphen_report(path: Path, rows: list[dict[str, str]]) -> str:
    known = set()
    for row in rows:
        for column in ("usb_vid", "usb_pid", "l6_product"):
            if row[column] != EMPTY:
                known.add(int(row[column], 16))
    unsourced_values: set[int] = set()
    rows_with_unsourced = 0
    total = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        total += 1
        found = [int(value, 16) for value in re.findall(r"0x[0-9A-Fa-f]+", line)]
        missing = [value for value in found if value not in known]
        if missing:
            rows_with_unsourced += 1
            unsourced_values.update(missing)
    return (
        f"hyphen rows {total}; rows with an id absent from this table "
        f"{rows_with_unsourced}; distinct absent ids {len(unsourced_values)}"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-mac", type=Path, required=True)
    parser.add_argument("--java-windows", type=Path, required=True)
    parser.add_argument("--monkey", type=Path, required=True)
    parser.add_argument("--names", type=Path, required=True)
    parser.add_argument("--hyphen", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    mac_constants, mac_named = parse_java(args.java_mac)
    windows_constants, windows_named = parse_java(args.java_windows)
    if mac_constants != windows_constants or mac_named != windows_named:
        raise SystemExit("Mac and Windows L6ProductConstants disagree")
    names = [line.strip() for line in args.names.read_text(encoding="utf-8").splitlines() if line.strip()]
    if len(names) != len(set(names)):
        raise SystemExit("owner name list has duplicate lines")
    rows = build(mac_named, mac_constants, parse_monkey(args.monkey), names)
    args.output.write_text(render(rows), encoding="utf-8")
    print(f"wrote {len(rows)} rows to {args.output}")
    print(hyphen_report(args.hyphen, rows))
    print(f"owner names {len(names)}; canonical names {len({row['name'] for row in rows if row['role'] != 'alias'})}")


if __name__ == "__main__":
    main()
