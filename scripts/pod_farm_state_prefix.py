"""Owned offline XFP6 prefix projection, not an importer or native oracle.

Only the recovered fixed-prefix discrimination is modeled. Advertised totals,
section bounds, nested formats, runtime identity and application are unverified.
No original state data or vendor code is included.
"""
import struct


def read_state_prefix(data):
    """Reconstruct the fixed 244-byte read; do not validate the whole state."""
    if len(data) < 0xf4:
        raise ValueError('short fixed state prefix')
    if data[:4] == b'XFP6':
        order = '<'
    elif data[:4] == b'6PFX':
        order = '>'
    else:
        raise ValueError('unrecognized state signature')
    words = struct.unpack_from(order+'27I', data)
    if words[1] < 0xf4:
        raise ValueError('declared header shorter than fixed prefix')
    return {
        'header_length': words[1], 'advertised_total': words[2],
        'numeric_words': words,
        'section_words': tuple((words[i], words[i+1]) for i in range(6, 14, 2)),
        'name_bytes': data[0x6c:0xec],
        'final_words': struct.unpack_from(order+'2I', data, 0xec),
    }


def vst_restore_status(result_word):
    """Model the i386 wrapper's sign-bit result projection, not restoration."""
    return int((result_word & 0x80000000) == 0)
