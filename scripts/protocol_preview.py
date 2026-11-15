"""Offline preview derived from Workbench 1.75. Does not open MIDI or USB devices.

Sources: L6VariaxDevice.EnableVariaxMIDI / SetXTLiveVariaxMessagesEnable /
Int2VXWord, L6VariaxPatchManager.RequestDevicePatch, L6MIDIManager._SendSysex.
These messages have not been validated against physical hardware.
"""

import json


def vx_word(value):
    """Encode an unsigned 24-bit word as six least-significant-first nibbles."""
    if not 0 <= value <= 0xFFFFFF:
        raise ValueError("value must fit in an unsigned 24-bit word")
    return bytes((value >> shift) & 15 for shift in range(0, 24, 4))


def variax_message(payload):
    return bytes.fromhex("f0 00 01 0c 07 00") + payload + b"\xf7"


def metadata_requests():
    # Original reads two blocks of 32 words, starting at word address 851.
    return [variax_message(b"\x3c" + vx_word(32) + vx_word(address))
            for address in (851, 883)]


if __name__ == "__main__":
    print(json.dumps({
        "mode": "offline preview; nothing sent; hardware unverified",
        "podxt_live_routing_enable": "f0 00 01 0c 03 23 01 f7",
        "variax_midi_enable": variax_message(b"\x2a\x01").hex(" "),
        "metadata_requests": [message.hex(" ") for message in metadata_requests()],
        "expected_metadata_response_bytes": 200,
        "podxt_live_sysex_chunk_bytes": 256,
        "notes": "Routing enable changes session state. Reading other presets in the original app also changes the active program. No flash-write commands are included.",
    }, indent=2))
