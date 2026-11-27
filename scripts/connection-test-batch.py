#!/usr/bin/env python3
"""Record repeated PODxt Live / Variax cable changes on macOS or Windows."""

import argparse
import csv
from datetime import datetime, timezone
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from tempfile import mkdtemp
from time import monotonic, sleep


def bounded_integer(minimum, maximum):
    def parse(value):
        number = int(value)
        if not minimum <= number <= maximum:
            raise argparse.ArgumentTypeError(f"Expected {minimum} to {maximum}.")
        return number

    return parse


def tshark_path():
    candidates = [
        shutil.which("tshark"),
        Path("/Applications/Wireshark.app/Contents/MacOS/tshark"),
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "Wireshark/tshark.exe",
    ]
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return str(candidate)
    raise ValueError("TShark was not found. Use --markers-only to record observations.")


def usb_interfaces(tshark):
    result = subprocess.run(
        [tshark, "-D"], capture_output=True, text=True, errors="replace", check=True, timeout=20
    )
    interfaces = {}
    for line in result.stdout.splitlines():
        match = re.match(r"(\d+)\. (\S+)", line)
        if match:
            number, name = match.groups()
            if re.search(r"(?:^|\\)USBPcap\d+$|^XHC\d*$|^usbmon\d+$", name, re.IGNORECASE):
                interfaces[number] = name
    return interfaces


def check_running(process):
    if process is not None and process.poll() is not None:
        raise ValueError("USB capture stopped before the batch finished. See capture.log.")


def ready_to_capture(process, path):
    deadline = monotonic() + 15
    while monotonic() < deadline:
        check_running(process)
        if path.exists() and path.stat().st_size:
            return
        sleep(0.1)
    raise ValueError("TShark did not start writing a USB capture. See capture.log.")


def run_batch(args, scenario, folder, tshark, interface):
    process = None
    with (folder / "events.csv").open("x", newline="", encoding="utf-8") as log:
        events = csv.writer(log)
        events.writerow(["utc", "scenario", "attempt", "event", "capture_source", "notes"])

        def record(attempt, event, notes=""):
            stamp = datetime.now(timezone.utc).isoformat(timespec="milliseconds")
            events.writerow([stamp, scenario, attempt, event, interface or "markers-only", notes])
            log.flush()

        trace = folder / "usb.pcapng"
        record(0, "batch-opened", f"{args.attempts} planned attempt(s)")
        try:
            print(f"\nScenario: {scenario}", flush=True)
            input("Close both Workbench apps before starting. Press Enter when ready. ")
            if interface:
                with (folder / "capture.log").open("x", encoding="utf-8") as capture_log:
                    process = subprocess.Popen(
                        [tshark, "-n", "-i", interface, "-a", f"duration:{args.seconds}",
                         "-w", str(trace)],
                        stdout=capture_log, stderr=subprocess.STDOUT,
                    )
                    ready_to_capture(process, trace)
                record(0, "usb-file-started")
                print(f"USB capture time limit is {args.seconds} seconds.", flush=True)
            else:
                print("Recording observations only. No USB packets will be saved.", flush=True)

            print("Get the PODxt ready with VDI connected. Keep USB connected during the cycles.", flush=True)
            if scenario == "closed":
                print("Leave both apps closed for this batch.", flush=True)
            else:
                name = "Line 6 Workbench 1.75" if scenario == "original" else "Variax Workbench"
                record(0, "app-launch-prompt", name)
                print(f"Open {name} now and let it settle.", flush=True)
            note = input("When ready, enter any starting notes or press Enter: ").strip()
            check_running(process)
            record(0, "rig-ready", note)
            for attempt in range(1, args.attempts + 1):
                input(f"[{attempt}/{args.attempts}] Unplug the guitar's VDI cable. Press Enter immediately. ")
                check_running(process)
                record(attempt, "vdi-unplugged")
                input(f"[{attempt}/{args.attempts}] After about 5 seconds, reconnect VDI and press Enter. ")
                check_running(process)
                record(attempt, "vdi-reconnected")
                note = input("What did the app or PODxt show? Enter to leave blank: ").strip()
                check_running(process)
                record(attempt, "observation", note)

            if process is not None:
                print("Leave the rig steady while the USB capture finishes.", flush=True)
                if process.wait(timeout=args.seconds + 15):
                    raise ValueError("TShark could not finish the USB capture. See capture.log.")
                result = subprocess.run(
                    [tshark, "-r", str(trace), "-c", "1", "-T", "fields", "-e", "frame.number"],
                    capture_output=True, text=True, errors="replace", check=True, timeout=20,
                )
                if not result.stdout.strip():
                    raise ValueError("USB capture contains zero packets. Check the selected controller.")
                record(0, "usb-file-saved", "At least one packet. Verify PODxt and endpoint coverage.")
            record(0, "batch-complete")
        except (KeyboardInterrupt, EOFError, OSError, ValueError, subprocess.SubprocessError):
            record(0, "batch-incomplete")
            raise
        finally:
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--list-usb", action="store_true", help="list available TShark USB interfaces")
    mode.add_argument("--usb-interface", help="USB interface name or number from --list-usb")
    mode.add_argument("--markers-only", action="store_true", help="record cable changes without USB packets")
    parser.add_argument("--scenario", choices=["compare", "closed", "original", "workbench"],
                        default="compare", help="default compare runs closed and then original")
    parser.add_argument("--attempts", type=bounded_integer(1, 20), default=3)
    parser.add_argument("--seconds", type=bounded_integer(15, 600), default=120,
                        help="capture duration in seconds, default 120")
    parser.add_argument("--out", type=Path, default=Path.home() / "Variax connection tests")
    args = parser.parse_args()

    try:
        tshark = interface = None
        if not args.markers_only:
            tshark = tshark_path()
            interfaces = usb_interfaces(tshark)
            if not interfaces:
                raise ValueError("No USB capture interface is available. Use --markers-only for observations.")
            if args.list_usb:
                for number, name in interfaces.items():
                    print(f"{number}. {name}")
                return 0
            interface = interfaces.get(args.usb_interface, args.usb_interface)
            if interface not in interfaces.values():
                raise ValueError("Select a USB interface shown by --list-usb.")
        args.out.mkdir(parents=True, exist_ok=True)
        folder = Path(mkdtemp(prefix=f"{args.scenario}-", dir=args.out.resolve()))
        print(f"Results folder: {folder}", flush=True)
        scenarios = ("closed", "original") if args.scenario == "compare" else (args.scenario,)
        for scenario in scenarios:
            destination = folder / scenario
            destination.mkdir()
            run_batch(args, scenario, destination, tshark, interface)
        return 0
    except (KeyboardInterrupt, EOFError):
        print("Batch cancelled. Any recorded results are incomplete.", file=sys.stderr)
        return 130
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Connection test: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
