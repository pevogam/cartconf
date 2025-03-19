import unittest
import os
import sys

# simple magic for using scripts within a source tree
basedir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if os.path.isdir(os.path.join(basedir, 'cartconf')):
    sys.path.append(basedir)

from cartconf.filters import Filter, NoOnlyFilter, OnlyFilter, NoFilter, JoinFilter, BlockFilter, Condition, NegativeCondition


class TestFilters(unittest.TestCase):

    def setUp(self):
        self.ctx = ['a', 'b', 'c']
        self.ctx_set = set(self.ctx)
        self.descendant_labels = set(['d', 'e', 'f'])

    def test_filter_match(self):
        f = Filter([[['a'], ['b']]])
        self.assertTrue(f.match(self.ctx, self.ctx_set))
        self.assertFalse(f.match(['x', 'y', 'z'], set(['x', 'y', 'z'])))

    def test_filter_might_match(self):
        f = Filter([[['a'], ['b']]])
        self.assertTrue(f.might_match(self.ctx, self.ctx_set, self.descendant_labels))
        self.assertFalse(f.might_match(['x', 'y', 'z'], set(['x', 'y', 'z']), self.descendant_labels))

    def test_no_only_filter(self):
        f = NoOnlyFilter([[['a'], ['b']]], 'line')
        self.assertEqual(f.line, 'line')
        self.assertTrue(f.match(self.ctx, self.ctx_set))
        self.assertFalse(f.match(['x', 'y', 'z'], set(['x', 'y', 'z'])))

    def test_only_filter(self):
        f = OnlyFilter([[['a'], ['b']]], 'line')
        self.assertTrue(f.is_irrelevant(self.ctx, self.ctx_set, self.descendant_labels))
        self.assertFalse(f.requires_action(self.ctx, self.ctx_set, self.descendant_labels))
        self.assertFalse(f.is_irrelevant(['x', 'y', 'z'], set(['x', 'y', 'z']), self.descendant_labels))
        self.assertTrue(f.requires_action(['x', 'y', 'z'], set(['x', 'y', 'z']), self.descendant_labels))
        self.assertFalse(f.requires_action(['a', 'b', 'c'], set(['a', 'b', 'c']), self.descendant_labels))

    def test_no_filter(self):
        f = NoFilter([[['a'], ['b']]], 'line')
        self.assertFalse(f.is_irrelevant(self.ctx, self.ctx_set, self.descendant_labels))
        self.assertTrue(f.requires_action(self.ctx, self.ctx_set, self.descendant_labels))
        self.assertTrue(f.is_irrelevant(['x', 'y', 'z'], set(['x', 'y', 'z']), self.descendant_labels))
        self.assertFalse(f.requires_action(['x', 'y', 'z'], set(['x', 'y', 'z']), self.descendant_labels))
        self.assertFalse(f.is_irrelevant(['a', 'b', 'c'], set(['a', 'b', 'c']), self.descendant_labels))

    def test_join_filter(self):
        f = JoinFilter([[['a'], ['b']]], 'line')
        self.assertEqual(str(f), "Join [[['a'], ['b']]]")
        self.assertEqual(repr(f), "Join [[['a'], ['b']]]")

    def test_block_filter(self):
        f = BlockFilter(['blocked'])
        self.assertEqual(f.blocked, ['blocked'])

    def test_condition(self):
        f = Condition([[['a'], ['b']]], 'line')
        self.assertEqual(str(f), "Condition [[['a'], ['b']]]:[]")
        self.assertEqual(repr(f), "Condition [[['a'], ['b']]]:[]")

    def test_negative_condition(self):
        f = NegativeCondition([[['a'], ['b']]], 'line')
        self.assertEqual(str(f), "NotCond [[['a'], ['b']]]:[]")
        self.assertEqual(repr(f), "NotCond [[['a'], ['b']]]:[]")

if __name__ == '__main__':
    unittest.main()
