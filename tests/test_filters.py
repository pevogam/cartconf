import unittest
import os
import sys

# simple magic for using scripts within a source tree
basedir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if os.path.isdir(os.path.join(basedir, 'cartconf')):
    sys.path.append(basedir)

from cartconf.filters import Filters, Filter, NoOnlyFilter, OnlyFilter, NoFilter, JoinFilter, BlockFilter, Condition, NegativeCondition
from cartconf.parser import Label, Lexer, Tokens


class TestFilters(unittest.TestCase):

    def setUp(self):
        self.ctx = [Label('a'), Label('b'), Label('c')]
        self.descendant_labels = [Label('d'), Label('e'), Label('f')]

    def test_filter_match(self):
        f = Filter([[[Label('a')], [Label('b')]]])
        self.assertTrue(f.match_ctx(self.ctx))
        self.assertFalse(f.match_ctx([Label('x'), Label('y'), Label('z')]))

    def test_filter_might_match(self):
        f = Filter([[[Label('a')], [Label('b')]]])
        self.assertTrue(f.might_match(self.ctx, self.descendant_labels))
        self.assertFalse(f.might_match([Label('x'), Label('y'), Label('z')], self.descendant_labels))

    def test_no_only_filter(self):
        f = NoOnlyFilter([[[Label('a')], [Label('b')]]], 'line')
        self.assertEqual(f.line, 'line')
        self.assertTrue(f.match_ctx(self.ctx))
        self.assertFalse(f.match_ctx([Label('x'), Label('y'), Label('z')]))

    def test_only_filter(self):
        f = OnlyFilter([[[Label('a')], [Label('b')]]], 'line')
        self.assertTrue(f.is_irrelevant(self.ctx, self.descendant_labels))
        self.assertFalse(f.requires_action(self.ctx, self.descendant_labels))
        self.assertFalse(f.is_irrelevant([Label('x'), Label('y'), Label('z')], self.descendant_labels))
        self.assertTrue(f.requires_action([Label('x'), Label('y'), Label('z')], self.descendant_labels))
        self.assertFalse(f.requires_action([Label('a'), Label('b'), Label('c')], self.descendant_labels))

    def test_no_filter(self):
        f = NoFilter([[[Label('a')], [Label('b')]]], 'line')
        self.assertFalse(f.is_irrelevant(self.ctx, self.descendant_labels))
        self.assertTrue(f.requires_action(self.ctx, self.descendant_labels))
        self.assertTrue(f.is_irrelevant([Label('x'), Label('y'), Label('z')], self.descendant_labels))
        self.assertFalse(f.requires_action([Label('x'), Label('y'), Label('z')], self.descendant_labels))
        self.assertFalse(f.is_irrelevant([Label('a'), Label('b'), Label('c')], self.descendant_labels))

    def test_join_filter(self):
        f = JoinFilter([[[Label('a')], [Label('b')]]], 'line')
        self.assertEqual(str(f), "Join [[[a], [b]]]")
        self.assertEqual(repr(f), "Join [[[a], [b]]]")

    def test_block_filter(self):
        f = BlockFilter(True)
        self.assertEqual(f.blocked, True)

    def test_condition(self):
        f = Condition([[[Label('a')], [Label('b')]]], 'line')
        self.assertEqual(str(f), "Condition [[[a], [b]]]")
        self.assertEqual(repr(f), "Condition [[[a], [b]]]")

    def test_negative_condition(self):
        f = NegativeCondition([[[Label('a')], [Label('b')]]], 'line')
        self.assertEqual(str(f), "NotCond [[[a], [b]]]")
        self.assertEqual(repr(f), "NotCond [[[a], [b]]]")

    def test_match_adjacent_basic(self):
        block = [Label('a')]
        ctx = [Label('a'), Label('b')]
        self.assertEqual(Filters.Filter.match_adjacent(block, ctx), 1)

    def test_match_adjacent_full_match(self):
        block = [Label('a'), Label('b')]
        ctx = [Label('a'), Label('b'), Label('c')]
        self.assertEqual(Filters.Filter.match_adjacent(block, ctx), 2)

    def test_match_adjacent_no_match(self):
        block = [Label('x')]
        ctx = [Label('a'), Label('b')]
        self.assertEqual(Filters.Filter.match_adjacent(block, ctx), 0)

    def test_match_adjacent_empty_context(self):
        block = [Label('a'), Label('b')]
        ctx = []
        self.assertEqual(Filters.Filter.match_adjacent(block, ctx), 0)

    def test_might_match_adjacent_true(self):
        block = [Label('a'), Label('b'), Label('e')]
        ctx = [Label('a'), Label('b')]
        descendant_labels = [Label('d'), Label('e')]
        self.assertTrue(Filters.Filter.might_match_adjacent(block, ctx, descendant_labels))

    def test_might_match_adjacent_false(self):
        block = [Label('a'), Label('x')]
        ctx = [Label('a'), Label('b')]
        descendant_labels = [Label('d'), Label('e')]
        self.assertFalse(Filters.Filter.might_match_adjacent(block, ctx, descendant_labels))

    def test_might_match_adjacent_empty_context(self):
        block = [Label('a'), Label('b')]
        ctx = []
        descendant_labels = [Label('d'), Label('e')]
        self.assertEqual(Filters.Filter.might_match_adjacent(block, ctx, descendant_labels), False)

    def test_parse_filter(self):
        lexer = Lexer(content="test.value")
        lexer.set_prev_indent(-1)
        tokens = lexer.get_until([Tokens.LEndL])
        filters = Filters.parse_filter(tokens[1:], lexer.line, lexer.filename, lexer.linenum)
        self.assertEqual(len(filters), 1)
        self.assertEqual(len(filters[0]), 1)
        self.assertEqual(len(filters[0][0]), 2)
        self.assertEqual(filters[0][0][0].name, "test")
        self.assertEqual(filters[0][0][1].name, "value")

    def test_parse_filter_complicated(self):
        f = "only xxx.yyy..(xxx=333).aaa, ddd (eeee) rrr.aaa"
        lexer = Lexer(content=f)
        lexer.set_prev_indent(-1)
        lexer.get_next_token([Tokens.LIndent])
        lexer.get_next_token([Tokens.LOnly])
        rest_tokens = lexer.get_rest_line()
        p_filter = Filters.parse_filter(rest_tokens, lexer.line, lexer.filename, lexer.linenum)
        self.assertEqual(p_filter,
                         [[[Label("xxx"),
                            Label("yyy")],
                           [Label("xxx", "333"),
                            Label("aaa")]],
                          [[Label("ddd")]],
                          [[Label("eeee")]],
                          [[Label("rrr"),
                            Label("aaa")]]],
                         "Failed to parse filter.")


if __name__ == '__main__':
    unittest.main()
