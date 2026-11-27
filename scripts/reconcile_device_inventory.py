#!/usr/bin/env python3
"""Emit the #114 offline names-and-roles comparison from pinned Git blobs."""

import csv
import hashlib
import io
import json
import subprocess
import sys
from collections import Counter, defaultdict

SNAPSHOT = "508cd000cc7f0cb074cde0056a8b1a52fab4cbdf"
MASTER = "309f948ceeda368123649f848526c7bc320632cf"
ROLE_COUNTS = {"alias": 186, "product": 29, "guitar": 5, "bridge": 2, "family": 2}


def git_blob(commit, path):
    return subprocess.check_output(["git", "show", f"{commit}:{path}"])


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def comparison():
    inputs = {}

    def read(label, commit, path):
        data = git_blob(commit, path)
        inputs[label] = {"commit": commit, "path": path, "sha256": sha256(data)}
        return data

    source = json.loads(read("coverage_source", MASTER, "research/audio-inventory-source.json"))
    projection = read("coverage_projection", MASTER, "research/audio-inventory-schema2.tsv")
    snapshot = read("snapshot", SNAPSHOT, "supported_devices.txt")
    read("historical_generator", SNAPSHOT, "scripts/build_supported_devices.py")
    underscore = read("underscore_draft", MASTER, "supported_devices.txt")
    hyphen = read("hyphen_draft", MASTER, "supported-devices.txt")
    require(sha256(snapshot) == source["original_reported_sha256"], "snapshot hash mismatch")
    require(sha256(projection) == source["projection_sha256"], "projection hash mismatch")

    schema, table = snapshot.decode("utf-8").split("\n", 1)
    require(source["schema"] == 2 and schema == "schema\t2", "schema mismatch")
    rows = list(csv.DictReader(io.StringIO(table, newline=""), delimiter="\t"))
    names_and_roles = "name\trole\n" + "".join(
        f"{row['name']}\t{row['role']}\n" for row in rows
    )
    require(names_and_roles.encode("utf-8") == projection, "ordered projection mismatch")
    roles = Counter(row["role"] for row in rows)
    require(
        len(rows) == source["rows"] == 224
        and roles == ROLE_COUNTS
        and roles["alias"] == source["alias_rows"]
        and len(rows) - roles["alias"] == source["reported_recovered_rows"],
        "row or role count mismatch",
    )
    require(
        all(
            row[column] == "-"
            for row in rows
            if row["role"] == "alias"
            for column in ("usb_vid", "usb_pid", "l6_product")
        ),
        "snapshot alias carries an identifier",
    )

    by_name = defaultdict(lambda: {"snapshot": [], "underscore_lines": [], "hyphen_lines": []})
    for ordinal, row in enumerate(rows, 1):
        by_name[row["name"]]["snapshot"].append({"ordinal": ordinal, "role": row["role"]})
    underscore_lines = underscore.decode("utf-8").splitlines()
    for line_number, name in enumerate(underscore_lines, 1):
        require(name and name == name.strip(), f"malformed underscore line {line_number}")
        by_name[name]["underscore_lines"].append(line_number)

    hyphen_lines = hyphen.decode("utf-8").splitlines()
    hyphen_statuses = Counter()
    for line_number, line in enumerate(hyphen_lines, 1):
        fields = line.split(" | ")
        require(
            len(fields) == 4
            and fields[0]
            and fields[1].startswith("USB=")
            and fields[2].startswith("L6=")
            and fields[3] in {"partial", "referenced-only"},
            f"malformed hyphen line {line_number}",
        )
        name, _, _, status = fields
        by_name[name]["hyphen_lines"].append(line_number)
        hyphen_statuses[status] += 1

    ordered_names = sorted(by_name, key=lambda name: (name.casefold(), name))
    groups = defaultdict(list)
    for name in ordered_names:
        key = "".join(character for character in name.casefold() if character.isalnum())
        groups[key].append(name)
    report = {
        "schema": 2,
        "scope": "Offline catalog comparison. Spellings and catalog roles confer no device identity.",
        "inputs": inputs,
        "row_counts": {
            "snapshot": len(rows),
            "underscore_draft": len(underscore_lines),
            "hyphen_draft": len(hyphen_lines),
        },
        "snapshot_role_counts": roles,
        "hyphen_draft_status_counts": hyphen_statuses,
        "exact_names": {name: by_name[name] for name in ordered_names},
        "draft_names_missing_from_snapshot": [
            name for name in ordered_names if not by_name[name]["snapshot"]
        ],
        "snapshot_names_missing_from_both_drafts": [
            name for name in ordered_names
            if by_name[name]["snapshot"]
            and not (by_name[name]["underscore_lines"] or by_name[name]["hyphen_lines"])
        ],
        "hyphen_only_draft_names": [
            name for name in ordered_names
            if by_name[name]["hyphen_lines"] and not by_name[name]["underscore_lines"]
        ],
        "exact_duplicates": {
            field: [name for name in ordered_names if len(by_name[name][field]) > 1]
            for field in ("snapshot", "underscore_lines", "hyphen_lines")
        },
        "spelling_collision_candidates": [names for names in groups.values() if len(names) > 1],
    }
    # Keep the report byte-identical on platforms with different native newlines.
    return (json.dumps(report, indent=2) + "\n").encode("ascii")


if __name__ == "__main__":
    sys.stdout.buffer.write(comparison())
