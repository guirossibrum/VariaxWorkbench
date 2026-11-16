"""Research-only reconstruction of the scalar-copy core at x64 VA 0x40d80.

Synthetic validation of derived copy facts, not a vendor execution oracle or a
PODxt parameter API. Excludes callbacks, model-specific exceptions and refresh.
Never interpret these in-memory definition records as persisted preset bytes.
"""
import struct


def project_words(definition, current_word):
    """Return copied words for known scalar tags, without assuming control units.

    The source record is synthetic/offline data. The meaning of the source word
    at 0x134 remains unverified; it is kept distinct from the supplied current
    word. Unknown types are rejected by this research decoder even though the
    native routine has an additional, not semantically classified fallback.
    """
    if len(definition) < 0x138:
        raise ValueError('truncated native definition record')
    if not 0 <= current_word <= 0xffffffffffffffff:
        raise ValueError('current word is not u64')
    tag, lower, upper, word134 = struct.unpack_from('<4I', definition, 0x128)
    if tag not in (0, 1, 0x10):
        raise ValueError('unclassified native scalar tag')
    def convert(word):
        word &= 0xffffffff
        if tag == 0 and word & 0x80000000:
            return word | 0xffffffff00000000
        return word
    return {'scalar_tag': tag, 'flags118': {0: 0x10000000, 1: 0, 0x10: 0x20000000}[tag],
            'word120': convert(lower), 'word128': convert(upper),
            'word130_current': convert(current_word), 'word138_from134': convert(word134)}
