import unittest
import os
import sys

# simple magic for using scripts within a source tree
basedir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if os.path.isdir(os.path.join(basedir, 'cartconf')):
    sys.path.append(basedir)

from cartconf.utils import convert_data_size, compare_string, apply_suffix_bounds, drop_suffixes


class TestUtils(unittest.TestCase):

    def test_convert_data_size(self):
        self.assertEqual(convert_data_size('1B'), 1)
        self.assertEqual(convert_data_size('1K'), 1024)
        self.assertEqual(convert_data_size('1M'), 1024 * 1024)
        self.assertEqual(convert_data_size('1G'), 1024 * 1024 * 1024)
        self.assertEqual(convert_data_size('1T'), 1024 * 1024 * 1024 * 1024)
        self.assertEqual(convert_data_size('1'), 1)
        self.assertEqual(convert_data_size('1', 'K'), 1024)

    def test_compare_string(self):
        self.assertEqual(compare_string('1B', '1B'), 0)
        self.assertEqual(compare_string('1K', '1B'), 1)
        self.assertEqual(compare_string('1B', '1K'), -1)
        self.assertEqual(compare_string('1M', '1024K'), 0)
        self.assertEqual(compare_string('1G', '1024M'), 0)
        self.assertEqual(compare_string('1T', '1024G'), 0)
        self.assertEqual(compare_string('1', '1'), 0)
        self.assertEqual(compare_string('2', '1'), 1)
        self.assertEqual(compare_string('1', '2'), -1)
        self.assertEqual(compare_string('1.5G', '1.5G'), 0)
        self.assertEqual(compare_string('2G', '1.5G'), 1)
        self.assertEqual(compare_string('1.5G', '2G'), -1)

    def test_apply_suffix_bounds(self):
        d = {
            'size_max': '2G',
            'size_min': '1G',
            'size': '2.5G',
            'speed_fixed': '100M',
            'speed': '50M',
        }
        apply_suffix_bounds(d)
        self.assertEqual(d['size'], '2G')
        self.assertEqual(d['speed'], '100M')
        d['size'] = '0.5G'
        apply_suffix_bounds(d)
        self.assertEqual(d['size'], '1G')
        d['size'] = '1.5G'
        apply_suffix_bounds(d)
        self.assertEqual(d['size'], '1.5G')

    def test_drop_suffixes(self):
        d = {
            ('size', 'max'): '2G',
            ('size', 'min'): '1G',
            ('size',): '1.5G',
            ('speed', 'fixed'): '100M',
            ('speed',): '50M',
        }
        result = drop_suffixes(d)
        self.assertEqual(result['size'], '1.5G')
        self.assertEqual(result['speed'], '50M')

if __name__ == '__main__':
    unittest.main()
