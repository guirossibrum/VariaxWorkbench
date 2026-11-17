"""Synthetic/static design checks only; no device, vendor renderer or font load."""
import json
from pathlib import Path
import struct
import unittest
import xml.etree.ElementTree as ET

from audit_visual_resource_facts import font_metrics
from build_visual_contract_mockups import mapping, states

ROOT = Path(__file__).resolve().parents[1]


def luminance(hex_color):
    channels = [int(hex_color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [x / 12.92 if x <= .04045 else ((x + .055) / 1.055) ** 2.4 for x in channels]
    return sum(a * b for a, b in zip(linear, (.2126, .7152, .0722)))


class VisualContractTests(unittest.TestCase):
    def setUp(self):
        self.contract = json.loads((ROOT / 'docs/visual/component-contract.json').read_text())

    def test_layouts_have_contiguous_chrome_and_bounded_nonoverlapping_panels(self):
        for layout in self.contract['layouts']:
            width, height = layout['width'], layout['height']
            rectangles = list(layout['panels'].values())
            self.assertEqual(sum(w * h for _, _, w, h in rectangles), width * height)
            for i, (x, y, w, h) in enumerate(rectangles):
                self.assertTrue(0 <= x and 0 <= y and w > 0 and h > 0)
                self.assertLessEqual(x + w, width)
                self.assertLessEqual(y + h, height)
                for xx, yy, ww, hh in rectangles[i + 1:]:
                    self.assertTrue(x + w <= xx or xx + ww <= x or y + h <= yy or yy + hh <= y)
            for scale in self.contract['scales']:
                # Arithmetic proposal only, not rendered DPI verification.
                self.assertEqual(width * height * scale ** 2,
                                 sum(w * scale * h * scale for _, _, w, h in rectangles))

    def test_contrast_targets_on_proposed_opaque_pairs(self):
        self.assertAlmostEqual((luminance('#ffffff') + .05) / (luminance('#000000') + .05), 21)
        for front, back, minimum in self.contract['contrast_pairs']:
            a, b = sorted([luminance(self.contract['colors'][front]), luminance(self.contract['colors'][back])])
            self.assertGreaterEqual((b + .05) / (a + .05), minimum)

    def test_original_boards_are_reproducible_asset_free_and_bounded(self):
        for filename, generated in [('workflow-comparison.svg', mapping()), ('responsive-states.svg', states())]:
            source = (ROOT / 'docs/visual' / filename).read_text()
            self.assertEqual(source, generated)
            root = ET.fromstring(source)
            width, height = float(root.attrib['width']), float(root.attrib['height'])
            for element in root.iter():
                self.assertNotIn(element.tag.rsplit('}', 1)[-1], ('script', 'image', 'foreignObject', 'use'))
                self.assertFalse(any('href' in k or k.lower().startswith('on') for k in element.attrib))
                if element.tag.endswith('rect'):
                    x, y = float(element.get('x', 0)), float(element.get('y', 0))
                    self.assertTrue(x >= 0 and y >= 0)
                    self.assertLessEqual(x + float(element.get('width')), width)
                    self.assertLessEqual(y + float(element.get('height')), height)

    def test_state_and_focus_contract_coverage(self):
        self.assertEqual({x['id'] for x in self.contract['states']},
                         {'dirty', 'save_success', 'save_failure', 'cancel', 'discard', 'empty', 'unsupported', 'hardware_locked'})
        self.assertEqual(self.contract['modal_focus'], ['save_copy', 'discard', 'cancel'])
        self.assertEqual(len(set(self.contract['focus_order'])), 7)
        self.assertGreaterEqual(self.contract['tokens']['target'], 32)

    def test_synthetic_sfnt_fields_and_malformed_directory(self):
        data = bytearray(300)
        struct.pack_into('>4sH', data, 0, b'\0\1\0\0', 3)
        for i, (tag, offset, size) in enumerate([(b'head', 64, 20), (b'hhea', 96, 10), (b'OS/2', 128, 78)]):
            struct.pack_into('>4sIII', data, 12 + i * 16, tag, 0, offset, size)
        struct.pack_into('>H', data, 64 + 18, 2048)
        struct.pack_into('>hhh', data, 96 + 4, 1900, -400, 0)
        self.assertEqual(font_metrics(data)['units_per_em'], 2048)
        self.assertEqual(font_metrics(data)['hhea_ascent_descent_gap'], (1900, -400, 0))
        for broken in (data[:30], data[:180], b'not a font'):
            with self.assertRaises(ValueError):
                font_metrics(broken)
        duplicate = bytearray(data)
        duplicate[28:32] = b'head'
        with self.assertRaises(ValueError):
            font_metrics(duplicate)


if __name__ == '__main__':
    unittest.main()
