"""Owned table-free legacy scalar reconstructions from static x86-64 evidence.

No vendor table/code/fixture, native execution or device API. Integer/byte inputs
represent source-width words; no claim of units, defaults or product applicability.
"""


def signed_word(value):
    value &= 0xffffffff
    return value if value < 0x80000000 else value - 0x100000000


def clamp_integer_byte(word):
    """AU 0x1a58ec encode: signed 32-bit word clamped into 0..127."""
    return max(0, min(127, signed_word(word)))


def decode_permutation(byte):
    """AU 0x1a591e decode: raw byte, including values above 127."""
    return {0: 2, 1: 3, 2: 1}.get(byte & 0xff, 0)


def encode_permutation(word):
    value = signed_word(word)
    return {0: 3, 1: 2, 2: 0}.get(value, 3 if value < 0 else 1)


def decode_three_level(byte):
    value = byte & 0xff
    return 0 if value < 42 else 1 if value < 86 else 2


def encode_three_level(word):
    """AU 0x1a5988 encode: source instruction shifts in 32 bits before clamp."""
    return clamp_integer_byte((word << 6) & 0xffffffff)


def decode_boolean(byte):
    return int((byte & 0xff) > 63)


def encode_boolean(word):
    return 127 if word & 0xffffffff else 0


def legacy_category(length, high_byte, low_byte):
    """AU 0x1915fc category only; caller already discriminated signature/length.

    Restrict to the two recovered record lengths as an owned helper precondition;
    this restriction does not describe the native routine's else-branch behavior.
    """
    if length not in (168, 170):
        raise ValueError('helper requires a signature-validated 168/170-byte record')
    if length == 168:
        return 0x400005
    bank = (high_byte & 0xff) * 128 + (low_byte & 0xff)
    if 64 <= bank < 128 or 256 <= bank < 320:
        return 0x400003
    if 128 <= bank < 192 or 320 <= bank < 384:
        return 0x400002
    return 0x400005
