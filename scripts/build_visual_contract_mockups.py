"""Original synthetic SVG diagrams of the proposed component contracts.

No vendor imagery, typography, glyph paths, resources or runtime is used.
"""
import html
from pathlib import Path
import textwrap


def svg(title, height):
    return [f'<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="{height}" viewBox="0 0 1280 {height}" role="img" aria-labelledby="title desc">',
            f'<title id="title">{html.escape(title)}</title>',
            '<desc id="desc">Original specification diagram with synthetic data. Proposed behavior, not implemented GUI or a vendor screenshot.</desc>',
            '<style>text{font-family:sans-serif;font-size:14px;fill:#ebebeb}.title{font-size:18px;font-weight:bold}.muted{fill:#c0c0c0}</style>',
            f'<rect width="1280" height="{height}" fill="#000000"/>']


def text(out, x, y, value, cls=''):
    out.append(f'<text x="{x}" y="{y}" class="{cls}">{html.escape(value)}</text>')


def box(out, x, y, w, h, fill='#222222', stroke='none'):
    out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{fill}" stroke="{stroke}" stroke-width="2"/>')


def paragraph(out, x, y, value, width=65):
    for i, line in enumerate(textwrap.wrap(value, width)):
        text(out, x, y + i * 20, line)


def mapping():
    out = svg('Annotated visual hierarchy comparison', 760)
    text(out, 24, 32, 'REFERENCE HIERARCHY / GUITAR / POD', 'title')
    text(out, 24, 57, 'Original schematics. Reference column reconstructs XML hierarchy only; all task behavior is proposed.', 'muted')
    for i, (title, context, canvas, inspector, library) in enumerate([
        ('Resource reconstruction', '60 / 48 authored-unit bars', 'Task canvas / custom composition unknown', 'Category + parameter panes', '250 authored-unit utility pane'),
        ('Variax local editor proposal', 'T-Model / Bank 1 / Slot 2 / Unsaved', 'Modeled virtual pickups (not piezo sensors)', 'Body | Pickups | Wiring | Tuning', 'Local guitar documents'),
        ('POD local library proposal', 'PODxt Live / Tone A / source file', 'Recovered metadata; unsupported editing', 'Raw model ID + unverified reason', 'Family-scoped local tones')]):
        x = 24 + i * 416
        text(out, x, 96, title, 'title')
        box(out, x, 112, 400, 44, '#0f0f0f')
        text(out, x + 12, 140, '1  Global navigation / offline scope')
        box(out, x, 164, 400, 44)
        text(out, x + 12, 192, '2  ' + context)
        box(out, x, 216, 286, 224, '#0f0f0f')
        paragraph(out, x + 12, 244, '3  ' + canvas, 30)
        if i == 1:
            out.append(f'<path d="M{x+50} 340 Q{x+20} 300 {x+100} 308 L{x+250} 308 L{x+250} 336 L{x+100} 336 Q{x+20} 390 {x+50} 340Z" fill="#424242" stroke="#c0c0c0"/>')
            for dx in (110, 156):
                box(out, x + dx, 309, 12, 26, '#78dce8')
            text(out, x + 12, 405, 'Original symbolic guitar geometry', 'muted')
        box(out, x + 294, 216, 106, 414)
        paragraph(out, x + 304, 244, '5  ' + library, 11)
        box(out, x, 448, 286, 182, '#0f0f0f')
        paragraph(out, x + 12, 476, '4  ' + inspector, 30)
        paragraph(out, x + 12, 556, 'Named value + units. Numeric entry accompanies any knob.', 30)
        box(out, x, 638, 400, 44)
        text(out, x + 12, 665, '6  Persistent status / capability reason')
    text(out, 24, 718, 'Proposed full layout: 60 / 48 / 24 chrome, 250 library; 4/8/12/16/24 spacing; 32 minimum targets.', 'muted')
    out.append('</svg>')
    return '\n'.join(out) + '\n'


def states():
    out = svg('Save, capability and responsive state contracts', 1040)
    text(out, 24, 32, 'STATE CONTRACTS / ORIGINAL SYNTHETIC REVIEW BOARD', 'title')
    cards = [
        ('Dirty document / blocking prompt', 'Unsaved: T-Model - rehearsal', 'Save new copy | Discard | Cancel', 'Tab cycles only these three actions. Default focus: Cancel; restore invoker on cancel. Background edit and navigation stay blocked.'),
        ('Failed save / keep work', 'Copy not saved: destination exists', 'Choose new path | Cancel', 'Keep document dirty and retain the pending action. Never imply overwrite or successful save. No operation continues after failure.'),
        ('Successful save / pending action', 'Saved new copy: rehearsal-02.vwp', 'Continue guarded pending action', 'Success text appears only after the existing atomic save API returns success. A cancelled picker is not success.'),
        ('Empty local library', 'No local presets in this family', 'Open local file', 'A malformed file preserves the prior document and selection. Search and empty results never imply cross-family conversion.'),
        ('Unsupported model / hardware locked', 'Parameter definition unverified', 'Raw ID visible / hardware access locked', 'Separate unsupported editing from offline hardware lock. Both reasons are reachable by keyboard. No backend initialization.'),
        ('Compact layout / 150% and 200%', '1024 x 768 logical: Library drawer', '32 target / 20 line / 2 focus border', 'Stack inspector columns; keep values and units visible. Long labels wrap, filenames reveal full paths on focus. No text below 14.')]
    for i, (title, message, actions, note) in enumerate(cards):
        x = 24 + (i % 2) * 628
        y = 68 + (i // 2) * 310
        box(out, x, y, 604, 286, '#0f0f0f')
        text(out, x + 16, y + 30, title, 'title')
        box(out, x + 16, y + 48, 572, 44, '#222222', '#78dce8')
        text(out, x + 28, y + 76, message)
        text(out, x + 16, y + 124, actions)
        paragraph(out, x + 16, y + 162, note, 64)
        text(out, x + 16, y + 264, 'Proposed contract; verify future egui implementation separately.', 'muted')
    text(out, 24, 1020, 'Focus is an outline; selection has a marker + label. Neither is conveyed by color alone.', 'muted')
    out.append('</svg>')
    return '\n'.join(out) + '\n'


def main():
    from inspect_firmware import preserve
    root = Path(__file__).resolve().parents[1] / 'docs/visual'
    for name, content in [('workflow-comparison.svg', mapping()), ('responsive-states.svg', states())]:
        preserve(root / name, content.encode())
        print(root / name)


if __name__ == '__main__':
    main()
