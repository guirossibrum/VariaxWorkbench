"""Bounded read-only preset-metadata probe through PODxt Live interface 1.

Temporarily claims only the Variax interface, never the POD audio interface.
Allowed commands are identity, session MIDI enable/disable, and two recovered
metadata reads. It does not switch presets, write flash, or update firmware.
The original kernel driver is reattached in finally.
"""
import argparse
import json
from pathlib import Path
import time

import usb.core
import usb.util

ROOT = Path(__file__).resolve().parents[1]
IDENTITY = bytes.fromhex("f0 7e 7f 06 01 f7")
ENABLE = bytes.fromhex("f0 00 01 0c 07 00 2a 01 f7")
DISABLE = bytes.fromhex("f0 00 01 0c 07 00 2a 00 f7")
REQUESTS = [bytes.fromhex(s) for s in [
    "f0 00 01 0c 07 00 3c 00 02 00 00 00 00 03 05 03 00 00 00 f7",
    "f0 00 01 0c 07 00 3c 00 02 00 00 00 00 03 07 03 00 00 00 f7",
]]


def main():
    raise RuntimeError("Live USB probe locked by user request. Offline validation and explicit later authorization are required before any further device traffic.")
    # Retained for protocol review and offline simulation; intentionally unreachable.
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", action="store_true", help="also read current-preset metadata")
    args = parser.parse_args()
    directory = ROOT / "analysis/captures" / time.strftime("variax-usb-%Y%m%d-%H%M%S")
    directory.mkdir(parents=True, exist_ok=False)
    log = []
    device = usb.core.find(idVendor=0x0e41, idProduct=0x4650)
    if device is None:
        raise RuntimeError("PODxt Live not found")
    detached = claimed = activated = False
    buffer = bytearray()

    def send(message):
        assert message in [IDENTITY, ENABLE, DISABLE, *REQUESTS]
        log.append({"direction": "out", "message": message.hex(" ")})
        for i in range(0, len(message), 16):
            chunk = message[i:i+16]
            assert device.write(0x05, chunk, timeout=1000) == len(chunk)
            time.sleep(0.005)

    def receive(predicate, timeout=3.0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                data = bytes(device.read(0x86, 16, timeout=100))
            except usb.core.USBTimeoutError:
                continue
            log.append({"direction": "in", "packet": data.hex(" ")})
            for byte in data:
                if byte >= 0xf8:  # MIDI realtime events may occur inside SysEx.
                    continue
                if byte == 0xf0:
                    buffer.clear()
                if byte == 0xf0 or buffer:
                    buffer.append(byte)
                if len(buffer) > 8192:
                    raise RuntimeError("unexpectedly large SysEx response")
                if byte == 0xf7 and buffer:
                    message = bytes(buffer)
                    buffer.clear()
                    log.append({"direction": "in", "message": message.hex(" ")})
                    if predicate(message):
                        return message
        raise TimeoutError("Variax did not return the expected response")

    try:
        if device.is_kernel_driver_active(1):
            device.detach_kernel_driver(1)
            detached = True
        usb.util.claim_interface(device, 1)
        claimed = True
        device.set_interface_altsetting(interface=1, alternate_setting=1)
        send(IDENTITY)
        identity = receive(lambda b: b.startswith(bytes.fromhex("f0 7e 7f 06 02 00 01 0c")))
        (directory / "identity.syx").write_bytes(identity)
        print("Identity:", identity.hex(" "), flush=True)
        if len(identity) < 13 or identity[8:10] != b"\x07\x00":
            raise RuntimeError("response is not a Variax-family identity; metadata reads skipped")
        if args.metadata:
            send(ENABLE)
            activated = True
            time.sleep(0.1)
            for i, request in enumerate(REQUESTS, 1):
                send(request)
                reply = receive(lambda b: len(b) == 200 and b[:5] == bytes.fromhex("f0 00 01 0c 07") and b[5] in (0, 4) and b[6] == 0x3c)
                (directory / f"metadata-{i}.syx").write_bytes(reply)
                print(f"Metadata block {i}: {len(reply)} bytes", flush=True)
    finally:
        try:
            if activated:
                send(DISABLE)
        finally:
            if claimed:
                usb.util.release_interface(device, 1)
            if detached:
                device.attach_kernel_driver(1)
            usb.util.dispose_resources(device)
            (directory / "messages.json").write_text(json.dumps(log, indent=2) + "\n")
            print("Capture:", directory, flush=True)


if __name__ == "__main__":
    main()
