#!/usr/bin/python

import unittest
import os
import gzip
import sys
import collections
import tempfile

# simple magic for using scripts within a source tree
basedir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if os.path.isdir(os.path.join(basedir, 'cartconf')):
    sys.path.append(basedir)

from cartconf import parser


testdir = os.path.dirname(__file__)
testdatadir = os.path.join(testdir, 'data')


class LabelTest(unittest.TestCase):

    def test_initialization(self):
        label = parser.Label("test")
        self.assertEqual(label.name, "test")
        self.assertIsNone(label.var_name)
        self.assertEqual(label.long_name, "test")
        self.assertIsNotNone(label.hash_val)
        self.assertIsNone(label.hash_var)

        label_with_var = parser.Label("test", "var")
        self.assertEqual(label_with_var.name, "var")
        self.assertEqual(label_with_var.var_name, "test")
        self.assertEqual(label_with_var.long_name, "(test=var)")
        self.assertIsNotNone(label_with_var.hash_val)
        self.assertIsNotNone(label_with_var.hash_var)

    def test_str(self):
        label = parser.Label("test")
        self.assertEqual(str(label), "test")

        label_with_var = parser.Label("test", "var")
        self.assertEqual(str(label_with_var), "(test=var)")

    def test_repr(self):
        label = parser.Label("test")
        self.assertEqual(repr(label), "test")

        label_with_var = parser.Label("test", "var")
        self.assertEqual(repr(label_with_var), "(test=var)")

    def test_eq(self):
        label1 = parser.Label("test")
        label2 = parser.Label("test")
        label3 = parser.Label("test", "var")
        label4 = parser.Label("test", "var")

        self.assertTrue(label1 == label2)
        self.assertFalse(label1 == label3)
        self.assertFalse(label4 == label1)
        self.assertTrue(label3 == label4)

    def test_ne(self):
        label1 = parser.Label("test")
        label2 = parser.Label("test")
        label3 = parser.Label("test", "var")
        label4 = parser.Label("test", "var")

        self.assertFalse(label1 != label2)
        self.assertTrue(label1 != label3)
        self.assertTrue(label4 != label1)
        self.assertFalse(label3 != label4)

    def test_hash(self):
        label1 = parser.Label("test")
        label2 = parser.Label("test")
        label3 = parser.Label("test", "var")
        label4 = parser.Label("test", "var")

        self.assertEqual(hash(label1), label1.hash_internal(label1.name))
        self.assertEqual(hash(label1), label1.hash_val)
        self.assertEqual(hash(label3), label3.hash_internal(label3.name))
        self.assertEqual(hash(label3), label3.hash_val)
        self.assertIsNone(label1.hash_var)
        self.assertEqual(label3.hash_var, label3.hash_internal(label3.long_name))

        hash_name = label1.hash_internal

        self.assertEqual(hash_name(label1.name), hash_name(label2.name))
        self.assertEqual(hash_name(label1.long_name), hash_name(label2.long_name))
        self.assertNotEqual(hash_name(label1.name), hash_name(label3.name))
        self.assertNotEqual(hash_name(label1.long_name), hash_name(label3.long_name))
        self.assertEqual(hash_name(label3.name), hash_name(label4.name))
        self.assertEqual(hash_name(label3.long_name), hash_name(label4.long_name))

        self.assertGreater(hash_name(label3.long_name), hash_name(label3.name))


class NodeTest(unittest.TestCase):

    def test_initialization(self):
        node = parser.Node()
        self.assertEqual(node.var_name, [])
        self.assertEqual(node.name, [])
        self.assertEqual(node.filename, "")
        self.assertEqual(node.dep, [])
        self.assertEqual(node.get_content(), [])
        self.assertEqual(node.get_children(), [])
        self.assertEqual(node.labels, [])
        self.assertFalse(node.append_to_shortname)
        self.assertEqual(node.get_failed_cases(), [])
        self.assertFalse(node.default)

    def test_process_content_operators(self):
        """Check that operators are all kept in new content."""
        node = parser.Node()
        op1 = parser.LSet("a", "b")
        op2 = parser.LAppend("c", "d")
        op3 = parser.LPrepend("e", "f")
        content = [
            ("<string>", 1, op1),
            ("<string>", 2, op2),
            ("<string>", 3, op3)
        ]
        node.swap_content(content)

        ctx, labels = [], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, content)
        self.assertEqual(failed_filters, [])

    def test_process_content_only_filter(self):
        """Check that a context matching only filter is removed or else failed."""
        node = parser.Node()
        op = parser.LSet("a", "b")
        label_x = parser.Label("x")
        label_y = parser.Label("y")
        only = parser.OnlyFilter([[[label_x]]], "x")
        content = [
            ("<string>", 1, op),
            ("<string>", 2, only)
        ]
        node.swap_content(content)

        # remove as irrelevant if matches
        ctx, labels = [label_x], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, [("<string>", 1, op)])
        self.assertEqual(failed_filters, [])

        # consider as failed if does not match
        ctx, labels = [label_y], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, [("<string>", 1, op)])
        self.assertEqual(failed_filters, [("<string>", 2, only)])

        # postpone if ambiguous
        ctx, labels = [label_y], [label_x]
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op))
        self.assertEqual(new_content[1], ("<string>", 2, only))
        self.assertEqual(failed_filters, [])

    def test_process_content_no_filter(self):
        """Check that a context matching no filter is failed or else removed."""
        node = parser.Node()
        op = parser.LSet("a", "b")
        label_x = parser.Label("x")
        label_y = parser.Label("y")
        no = parser.NoFilter([[[label_x]]], "x")
        content = [
            ("<string>", 1, op),
            ("<string>", 2, no)
        ]
        node.swap_content(content)

        # consider as failed if matches
        ctx, labels = [label_x], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, [("<string>", 1, op)])
        self.assertEqual(failed_filters, [("<string>", 2, no)])

        # remove as irrelevant if does not match
        ctx, labels = [label_y], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, [("<string>", 1, op)])
        self.assertEqual(failed_filters, [])

        # postpone if ambiguous
        ctx, labels = [label_y], [label_x]
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op))
        self.assertEqual(new_content[1], ("<string>", 2, no))
        self.assertEqual(failed_filters, [])

    def test_process_content_condition_filter(self):
        """Check that a context matching condition filter is unpacked or else not unpacked."""
        node = parser.Node()
        op1 = parser.LSet("a", "b")
        label_x = parser.Label("x")
        label_y = parser.Label("y")
        conditional_node = parser.Node()
        conditional_node.condition = parser.Condition([[[label_x]]], "x")
        op2 = parser.LSet("c", "d")
        conditional_node.add_content("<string>", 3, op2)
        content = [
            ("<string>", 1, op1),
            ("<string>", 2, conditional_node)
        ]
        node.swap_content(content)

        # unpack if matches
        ctx, labels = [label_x], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(new_content[1], ("<string>", 3, op2))
        self.assertEqual(failed_filters, [])

        # do not unpack if does not match
        ctx, labels = [label_y], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 1)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(failed_filters, [])

        # postpone if ambiguous
        ctx, labels = [label_y], [label_x]
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(new_content[1], ("<string>", 2, conditional_node))
        self.assertEqual(failed_filters, [])

    def test_process_content_negative_condition_filter(self):
        """Check that a context matching negative condition filter is unpacked or else not unpacked."""
        node = parser.Node()
        op1 = parser.LSet("a", "b")
        label_x = parser.Label("x")
        label_y = parser.Label("y")
        conditional_node = parser.Node()
        conditional_node.condition = parser.NegativeCondition([[[label_x]]], "x")
        op2 = parser.LSet("c", "d")
        conditional_node.add_content("<string>", 3, op2)
        content = [
            ("<string>", 1, op1),
            ("<string>", 2, conditional_node)
        ]
        node.swap_content(content)

        # unpack if does not match
        ctx, labels = [label_x], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 1)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(failed_filters, [])

        # do not unpack if matches
        ctx, labels = [label_y], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(new_content[1], ("<string>", 3, op2))
        self.assertEqual(failed_filters, [])

        # postpone if ambiguous
        ctx, labels = [label_y], [label_x]
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(new_content[1], ("<string>", 2, conditional_node))
        self.assertEqual(failed_filters, [])

    def test_process_content_nested_condition_with_operator(self):
        """Check that operators inside matched Condition blocks are unpacked correctly."""
        node = parser.Node()
        op1 = parser.LSet("a", "b")
        label_x = parser.Label("x")
        label_y = parser.Label("y")
        conditional_node = parser.Node()
        conditional_node.condition = parser.Condition([[[label_x]]], "x")
        op2 = parser.LSet("c", "d")
        conditional_node.add_content("<string>", 3, op2)
        nested_only = parser.OnlyFilter([[[label_y]]], "y")
        conditional_node.add_content("<string>", 4, nested_only)
        content = [
            ("<string>", 1, op1),
            ("<string>", 2, conditional_node)
        ]
        node.swap_content(content)

        # unpack if matches but expect nested filter to not match and fail the unpacking
        ctx, labels = [label_x], []
        new_content, failed_filters, failed_cond_filters = node.process_content(ctx, labels)
        self.assertEqual(len(new_content), 2)
        self.assertEqual(new_content[0], ("<string>", 1, op1))
        self.assertEqual(new_content[1], ("<string>", 3, op2))
        self.assertEqual(failed_filters, [("<string>", 2, conditional_node)])
        self.assertEqual(failed_cond_filters, [("<string>", 4, nested_only)])

    def test_process_content_empty(self):
        """Check that empty content is handled correctly."""
        node = parser.Node()
        label = parser.Label("x")

        ctx, labels = [], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, [])
        self.assertEqual(failed_filters, [])

        ctx, labels = [label], []
        new_content, failed_filters, _ = node.process_content(ctx, labels)
        self.assertEqual(new_content, [])
        self.assertEqual(failed_filters, [])

    def test_join_names(self):
        name1 = "test1.subtest1"
        name2 = "test1.subtest2"
        combined_name = parser.Node.join_names(name1, name2)
        self.assertEqual(combined_name, "test1.subtest1.subtest2")

    def test_dump(self):
        node = parser.Node()
        empty_dumped_str = node.dump(0)
        self.assertRegex(empty_dumped_str, r"name:.*\nvariable name:.*\ncontent:.*\nfailed cases:.*")

        node.name = [parser.Label("test_name")]
        node.var_name = [parser.Label("test_var_name")]
        node.add_content("test_content", 0, parser.LString("test_content"))
        failed_labels = [parser.Label("fail")]
        node.add_failed_case(failed_labels, [("<string>", 1, "str")], [], 5)
        dump_str = node.dump(2)
        expected_str = "  name: [test_name]\n  variable name: [test_var_name]\n  content: [ContentStep { filename: \"test_content\", linenum: 0, content_type: Tokens(LString(\"test_content\")) }]\n  failed cases: [([fail], [ContentStep { filename: \"<string>\", linenum: 1, content_type: String(\"str\") }], [])]"
        self.assertEqual(expected_str, dump_str)

    def test_dump_with_recurse(self):
        parent_node = parser.Node()
        child_node = parser.Node()
        child_node.name = [parser.Label("child_name")]
        parent_node.append_child(child_node)
        dump_str = parent_node.dump(0, recurse=True)
        expected_str = "name: []\nvariable name: []\ncontent: []\nfailed cases: []\n   name: [child_name]\n   variable name: []\n   content: []\n   failed cases: []"
        self.assertEqual(expected_str, dump_str)


class PreDictTest(unittest.TestCase):

    def setUp(self):
        self.pre_dict = parser.PreDict()

    def test_default_properties(self):
        pd = parser.PreDict()
        self.assertEqual(pd.ctx, [])
        self.assertEqual(pd.content, [])
        self.assertEqual(pd.shortname, [])
        self.assertEqual(pd.dep, [])
        self.assertEqual(pd.branch, [])
        self.assertEqual(pd.route, [])
        # str should include class name for debugging purposes
        self.assertIn(str(pd), "PreDict(ctx=[], content=[], shortname=[], dep=[])")

    def test_init_with_values(self):
        lab = parser.Label("a")
        op = parser.LSet("key", "value")
        pd = parser.PreDict(
            ctx=[lab],
            content=[("<string>", 1, op)],
            shortname=[lab],
            dep=["dep1"]
        )
        self.assertEqual(pd.ctx, [lab])
        self.assertEqual(pd.content, [("<string>", 1, op)])
        self.assertEqual(pd.shortname, [lab])
        self.assertEqual(pd.dep, ["dep1"])
        self.assertEqual(pd.branch, [])
        self.assertEqual(pd.route, [])

    def test_update_from_node_reset(self):
        pd = parser.PreDict()

        node = parser.Node()
        node.name = [parser.Label("n")]
        node.append_to_shortname = True
        node.dep = [[[parser.Label("dep1")]]]
        op = parser.LSet("k", "v")
        node.add_content("<file>", 1, op)

        # pre-dict is updated properly with a node
        updated = pd.update_from_node(node)
        self.assertTrue(updated)
        # ctx/shortname/dep/content should be updated accordingly
        self.assertEqual(pd.ctx, [parser.Label("n")])
        self.assertEqual(pd.shortname, [parser.Label("n")])
        self.assertEqual(pd.dep, ["dep1"])
        self.assertEqual(pd.content, [("<file>", 1, op)])
        # route should have grown by one entry
        self.assertEqual(pd.branch, [node])
        self.assertEqual(pd.route, [None])

        node2 = parser.Node()
        node2.name = [parser.Label("m")]
        node2.append_to_shortname = True
        node2.dep = [[[parser.Label("dep2")]]]
        op2 = parser.LSet("k2", "v2")
        node2.add_content("<file>", 2, op2)

        # pre-dict is updated additively with an extra node
        updated = pd.update_from_node(node2)
        self.assertTrue(updated)
        # ctx/shortname/dep/content should be updated accordingly
        self.assertEqual(pd.ctx, [parser.Label("n"), parser.Label("m")])
        self.assertEqual(pd.shortname, [parser.Label("n"), parser.Label("m")])
        self.assertEqual(pd.dep, ["dep1", "n.dep2"])
        self.assertEqual(pd.content, [("<file>", 1, op), ("<file>", 2, op2)])
        # route should have grown by one entry
        self.assertEqual(pd.branch, [node, node2])
        self.assertEqual(pd.route, [None, None])

        # pre-dict can then be reverted to updated state from previous node
        pd.reset_from_last_node()
        self.assertEqual(pd.ctx, [parser.Label("n")])
        self.assertEqual(pd.shortname, [parser.Label("n")])
        self.assertEqual(pd.dep, ["dep1"])
        self.assertEqual(pd.content, [("<file>", 1, op)])
        self.assertEqual(pd.branch, [node])
        self.assertIsNone(pd.route[-1])

    def test_update_from_node_failed(self):
        pd = parser.PreDict()

        node = parser.Node()
        node.name = [parser.Label("n")]
        node.append_to_shortname = True
        node.dep = [[[parser.Label("dep1")]]]
        op = parser.LSet("k", "v")
        node.add_content("<file>", 1, op)
        filter = parser.OnlyFilter([[[parser.Label("m")]]], "m")
        node.add_content("<file>", 2, filter)

        updated = pd.update_from_node(node)
        self.assertFalse(updated)
        # ctx/shortname/dep/content should be updated accordingly
        self.assertEqual(pd.ctx, node.name)
        self.assertEqual(pd.shortname, node.name)
        self.assertEqual(pd.dep, ["dep1"])
        self.assertEqual(pd.final_content, [("<file>", 1, op)])
        # route should still grow
        self.assertEqual(pd.branch, [node])
        self.assertEqual(pd.route, [None])

    def test_get_dict(self):
        lab = parser.Label("a")
        op = parser.LSet("key", "value")
        pd = parser.PreDict(
            ctx=[lab],
            content=[("<string>", 1, op)],
            shortname=[lab],
            dep=["dep1"]
        )
        self.assertEqual(
            pd.get_dict(),
            {"dep": ["dep1"], "key": "value", "name": "a", "shortname": "a"}
        )

        node = parser.Node()
        node.name = [parser.Label("n")]
        node.append_to_shortname = True
        node.dep = [[[parser.Label("dep2")]]]
        op = parser.LSet("key2", "value2")
        node.add_content("<file>", 1, op)
        updated = pd.update_from_node(node)
        self.assertTrue(updated)
        self.assertEqual(
            pd.get_dict(),
            {
                "name": "a.n",
                "shortname": "a.n",
                "dep": ["dep1", "a.dep2"],
                "key": "value",
                "key2": "value2",
            }
        )


class ReaderTest(unittest.TestCase):

    def test_initialization(self):
        s = "line1\nline2\n  line3\n"
        reader = parser.Reader(content=s)
        self.assertEqual(reader.filename, "<string>")
        self.assertEqual(len(reader.lines), 3)
        self.assertEqual(reader.lines[0], ("line1", 0, 1))
        self.assertEqual(reader.lines[1], ("line2", 0, 2))
        self.assertEqual(reader.lines[2], ("line3", 2, 3))

    def test_initialization_filename(self):
        with tempfile.NamedTemporaryFile() as temp_file:
            temp_file.write(b"line1\nline2\n  line3\n")
            temp_file.flush()
            temp_file_name = temp_file.name
            reader = parser.Reader(filename=temp_file_name)
        self.assertEqual(reader.filename, temp_file_name)
        self.assertEqual(len(reader.lines), 3)
        self.assertEqual(reader.lines[0], ("line1", 0, 1))
        self.assertEqual(reader.lines[1], ("line2", 0, 2))
        self.assertEqual(reader.lines[2], ("line3", 2, 3))

    def test_initialization_comments(self):
        s = "line1\nline2\n#line3\n  line4\n//line5\nline6\n"
        reader = parser.Reader(content=s)
        self.assertEqual(reader.filename, "<string>")
        self.assertEqual(len(reader.lines), 4)
        self.assertEqual(reader.lines[0], ("line1", 0, 1))
        self.assertEqual(reader.lines[1], ("line2", 0, 2))
        self.assertEqual(reader.lines[2], ("line4", 2, 4))
        self.assertEqual(reader.lines[3], ("line6", 0, 6))

    def test_initialization_tabs(self):
        s = "line1\nline2  \n  line3	\n"
        reader = parser.Reader(content=s)
        self.assertEqual(reader.filename, "<string>")
        self.assertEqual(len(reader.lines), 3)
        self.assertEqual(reader.lines[0], ("line1", 0, 1))
        self.assertEqual(reader.lines[1], ("line2", 0, 2))
        self.assertEqual(reader.lines[2], ("line3", 2, 3))

    def test_get_next_line(self):
        s = "line1\nline2\n  line3\nline4\n"
        reader = parser.Reader(content=s)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "line1")
        self.assertEqual(indent, 0)
        self.assertEqual(linenum, 1)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "line2")
        self.assertEqual(indent, 0)
        self.assertEqual(linenum, 2)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "line3")
        self.assertEqual(indent, 2)
        self.assertEqual(linenum, 3)
        line, indent, linenum = reader.get_next_line(2)
        self.assertEqual(line, None)
        self.assertEqual(indent, 0)
        self.assertEqual(linenum, 4)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "line4")
        self.assertEqual(indent, 0)
        self.assertEqual(linenum, 4)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, None)
        self.assertEqual(indent, -1)
        self.assertEqual(linenum, -1)

    def test_set_next_line(self):
        s = "line1\nline2\n  line3\n"
        reader = parser.Reader(content=s)
        reader.set_next_line("new line", 1, 4)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "new line")
        self.assertEqual(indent, 1)
        self.assertEqual(linenum, 4)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "line1")
        self.assertEqual(indent, 0)
        self.assertEqual(linenum, 1)


class LexerTest(unittest.TestCase):

    def setUp(self):
        self.sample_text = "variants: test\n  only test\n  no test\n  join test\n  suffix test\n  include test\n  del test\n  !test\n"
        self.lexer = parser.Lexer(content=self.sample_text)

    def test_initialization(self):
        self.assertEqual(self.lexer.filename, "<string>")
        self.assertIsNone(self.lexer.line)
        self.assertEqual(self.lexer.linenum, 0)
        self.assertFalse(self.lexer.ignore_white)
        self.assertFalse(self.lexer.rest_as_string)
        self.assertEqual(self.lexer.prev_indent, -1)

    def test_set_prev_indent(self):
        self.lexer.set_prev_indent(4)
        self.assertEqual(self.lexer.prev_indent, 4)

    def test_match_line(self):
        line = "only test"
        tokens = list(self.lexer.match_line(line, 0))
        self.assertEqual(len(tokens), 1)
        self.assertIsInstance(tokens[0], parser.LOnly)
        tokens = list(self.lexer.match_line(line, 0))
        self.assertEqual(len(tokens), 1)
        self.assertIsInstance(tokens[0], parser.LIdentifier)

    def test_get_next_token(self):
        next = self.lexer.get_next_token
        token = next()
        self.assertIsInstance(token, parser.LIndent)
        token = next()
        self.assertIsInstance(token, parser.LVariants)
        token = next()
        self.assertIsInstance(token, parser.LColon)
        token = next()
        self.assertIsInstance(token, parser.LWhite)
        self.assertEqual(token.string, " ")
        token = next()
        self.assertIsInstance(token, parser.LIdentifier)
        self.assertEqual(token.string, "test")

    def test_get_until(self):
        tokens = list(self.lexer.get_until([]))
        self.assertIsInstance(tokens[0], parser.LIndent)
        self.assertIsInstance(tokens[1], parser.LVariants)
        self.assertIsInstance(tokens[2], parser.LColon)
        self.assertIsInstance(tokens[3], parser.LWhite)
        self.assertIsInstance(tokens[4], parser.LIdentifier)
        self.assertIsInstance(tokens[5], parser.LEndL)

    def test_get_until_custom(self):
        tokens = self.lexer.get_until([parser.Tokens.default("only")])
        self.assertIsInstance(tokens[0], parser.LIndent)
        self.assertIsInstance(tokens[1], parser.LVariants)
        self.assertIsInstance(tokens[2], parser.LColon)
        self.assertIsInstance(tokens[3], parser.LWhite)
        self.assertIsInstance(tokens[4], parser.LIdentifier)
        self.assertIsInstance(tokens[5], parser.LEndL)
        self.assertIsInstance(tokens[6], parser.LIndent)
        self.assertIsInstance(tokens[7], parser.LOnly)

    def test_flush_until(self):
        self.lexer.flush_until([parser.Tokens.default("only")])
        token = self.lexer.get_next_token()
        self.assertIsInstance(token, parser.LIdentifier)
        self.assertEqual(token.string, "test")

    def test_get_until_check(self):
        tokens = self.lexer.get_until(
            [parser.Tokens.default("only")],
            [
                parser.Tokens.default("indent"),
                parser.Tokens.default("variants"),
                parser.Tokens.default(":"),
                parser.Tokens.default("WhiteSpace"),
                parser.Tokens.default("Identifier"),
                parser.Tokens.default("endl"),
            ],
        )
        self.assertIsInstance(tokens[0], parser.LIndent)
        self.assertIsInstance(tokens[1], parser.LVariants)
        self.assertIsInstance(tokens[2], parser.LColon)
        self.assertIsInstance(tokens[3], parser.LWhite)
        self.assertIsInstance(tokens[4], parser.LIdentifier)
        self.assertIsInstance(tokens[5], parser.LEndL)
        self.assertIsInstance(tokens[6], parser.LIndent)
        self.assertIsInstance(tokens[7], parser.LOnly)

        with self.assertRaises(parser.LexerError):
            self.lexer.get_until(
                [parser.Tokens.default("only")],
                [parser.Tokens.default(":")],
            )

    def test_get_until_no_white(self):
        tokens = self.lexer.get_until([parser.Tokens.default("only")], no_white=True)
        self.assertIsInstance(tokens[0], parser.LIndent)
        self.assertIsInstance(tokens[1], parser.LVariants)
        self.assertIsInstance(tokens[2], parser.LColon)
        self.assertIsInstance(tokens[3], parser.LIdentifier)
        self.assertIsInstance(tokens[4], parser.LEndL)
        self.assertIsInstance(tokens[5], parser.LIndent)
        self.assertIsInstance(tokens[6], parser.LOnly)

    def test_get_rest_line(self):
        tokens = self.lexer.get_rest_line()
        self.assertIsInstance(tokens[0], parser.LIndent)
        self.assertIsInstance(tokens[1], parser.LVariants)
        self.assertIsInstance(tokens[2], parser.LColon)
        self.assertIsInstance(tokens[3], parser.LWhite)

    def test_get_rest_line_no_white(self):
        tokens = self.lexer.get_rest_line(no_white=True)
        self.assertIsInstance(tokens[0], parser.LIndent)
        self.assertIsInstance(tokens[1], parser.LVariants)
        self.assertIsInstance(tokens[2], parser.LColon)
        # no white space token here
        self.assertIsInstance(tokens[3], parser.LIdentifier)

    def test_get_rest_line_as_string_token(self):
        self.lexer.get_next_token()  # indent
        self.lexer.get_next_token()  # variant
        self.lexer.get_next_token()  # colon
        token = self.lexer.get_rest_line_as_string_token()
        self.assertIsInstance(token, parser.LString)
        self.assertEqual(token.string, "test")

        # only compatible line endings are possible
        with self.assertRaises(parser.LexerError):
            self.lexer.get_rest_line_as_string_token()

    def test_get_next_token_check(self):
        token = self.lexer.get_next_token([parser.Tokens.default("indent")])
        self.assertIsInstance(token, parser.LIndent)

        with self.assertRaises(parser.LexerError):
            self.lexer.get_next_token([parser.Tokens.default("indent")])

    def test_get_next_token_check_nw(self):
        token = self.lexer.get_next_token([parser.Tokens.default("indent")], no_white=True)
        self.assertIsInstance(token, parser.LIndent)

        self.lexer.get_next_token([parser.Tokens.default("variants")], no_white=True)
        self.lexer.get_next_token([parser.Tokens.default(":")], no_white=True)
        # no white space token here
        self.lexer.get_next_token([parser.Tokens.default("Identifier")], no_white=True)
        self.lexer.get_next_token([parser.Tokens.default("endl")], no_white=True)

    def test_check_token(self):
        token = parser.LIdentifier("test")
        self.lexer.check_token(token, [parser.Tokens.default("Identifier")])
        with self.assertRaises(parser.LexerError):
            self.lexer.check_token(token, [parser.Tokens.default("indent")])


class ParserTest(unittest.TestCase):

    def setUp(self):
        self.parser = parser.Parser()
        self.maxDiff = None

    def test_initialization(self):
        self.assertIsInstance(self.parser.node, parser.Node)
        self.assertFalse(self.parser.debug)
        self.assertFalse(self.parser.defaults)
        self.assertEqual(self.parser.expand_defaults, [])
        self.assertIsNone(self.parser.filename)
        self.assertTrue(self.parser.parent_generator)

    def test_parse_file(self):
        with tempfile.NamedTemporaryFile() as temp_file:
            temp_file.write(b"variants:\n  - test:\n")
            temp_file.flush()
            temp_file_name = temp_file.name
            self.parser.parse_file(temp_file_name)
        self.assertEqual(self.parser.node.name, [])
        self.assertEqual(self.parser.node.get_content(), [])
        self.assertEqual(len(self.parser.node.get_children()), 1)
        self.assertEqual(self.parser.node.get_children()[0].name,
                         [parser.Label("test")])
        for c in self.parser.node.get_children()[0].get_content():
            self.assertEqual(c[0], temp_file_name)
        self.assertEqual(self.parser.filename, temp_file_name)

    def test_parse_string(self):
        test_string = "variants:\n  - test:\n"
        self.parser.parse_string(test_string)
        self.assertEqual(self.parser.node.name, [])
        self.assertEqual(self.parser.node.get_content(), [])
        self.assertEqual(len(self.parser.node.get_children()), 1)
        self.assertEqual(self.parser.node.get_children()[0].name,
                         [parser.Label("test")])
        for content_stage in self.parser.node.get_children()[0].get_content():
            self.assertEqual(content_stage[0], "<string>")
        self.assertIsNone(self.parser.filename)

    def test_get_dicts(self):
        self.parser.parse_string("variants:\n  - test:\n    key = value\n")
        dicts = list(self.parser.get_dicts())
        self.assertEqual(len(dicts), 1)
        self.assertEqual(dicts[0]["name"], "test")
        self.assertEqual(dicts[0]["_name_map_file"]["<string>"], "test")
        self.assertEqual(dicts[0]["key"], "value")

    def test_get_dicts_plain(self):
        self.parser.parse_string("variants:\n  - test:\n    key = value\n")
        parent_node = self.parser.node
        node = parent_node.get_children()[0]
        child_node = node.get_children()[0]

        d = self.parser.get_dicts_plain()
        self.assertEqual(d["name"], "test")
        self.assertEqual(d["_name_map_file"]["<string>"], "test")
        self.assertEqual(d["key"], "value")
        self.assertEqual(self.parser.get_dicts_plain(), d)

        # pre-dict cache stored after plain dictionary getter call
        pre_dict = parser.PreDict()
        self.assertEqual(self.parser.get_dicts_plain(pre_dict), d)
        cached_content = pre_dict.content
        self.assertEqual(len(cached_content), 3)
        self.assertEqual(pre_dict.branch, [parent_node, node, child_node])
        self.assertEqual(pre_dict.route, [0, 0, None])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict))

        # plain dictionary getter reuses pre-dict cache
        d["key2"] = "value2"
        op = parser.LSet("key2", "value2")
        self.parser.filename = "testfile"
        pre_dict = parser.PreDict(content=[(self.parser.filename, 3, op)])
        self.assertEqual(self.parser.get_dicts_plain(pre_dict), d)
        self.assertEqual(len(pre_dict.content), 4)
        self.assertEqual(pre_dict.content[0], (self.parser.filename, 3, op))
        self.assertEqual(pre_dict.content[1:], cached_content)

        # leaf node can only give one dictionary
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, child_node))
        self.assertEqual(pre_dict.route, [0, 0, None])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, child_node))
        self.assertEqual(pre_dict.route, [0, 0, None])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, child_node))
        self.assertEqual(pre_dict.route, [0, 0, None])

        # start node traverses the route completely as it finds no other dict
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, parent_node))
        self.assertEqual(pre_dict.route, [1])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, parent_node))
        self.assertEqual(pre_dict.route, [1])

        # intermediate node expands the route again then completes for its depth
        self.assertEqual(self.parser.get_dicts_plain(pre_dict, node), d)
        self.assertEqual(pre_dict.route, [1, 0, None])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, node))
        self.assertEqual(pre_dict.route, [1, 1])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict, node))
        self.assertEqual(pre_dict.route, [1, 1])

    def test_get_dicts_plain_failed(self):
        """Failed filters return empty dictionary with partial pre-dict."""
        self.parser.parse_string("variants:\n  - test1:\n    key1 = value1\n")
        op2 = parser.LSet("key2", "value2")
        self.parser.filename = "testfile"

        # case of redundant filter to be removed from pre-dict content
        filter1 = parser.OnlyFilter([[[parser.Label("test1")]]], "test1")
        pre_dict = parser.PreDict(
            content=[
                (self.parser.filename, 4, op2),
                (self.parser.filename, 5, filter1),
            ],
        )
        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "test1")
        self.assertEqual(d["_name_map_file"]["<string>"], "test1")
        self.assertEqual(d["key1"], "value1")
        self.assertEqual(d["key2"], "value2")
        self.assertIn((self.parser.filename, 5, filter1), pre_dict.content)
        self.assertNotIn((self.parser.filename, 5, filter1), pre_dict.final_content)
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict))

        # case of incompatible filter remaining in the pre-dict content
        filter2 = parser.OnlyFilter([[[parser.Label("test2")]]], "test2")
        pre_dict = parser.PreDict(
            content=[
                (self.parser.filename, 4, op2),
                (self.parser.filename, 5, filter2),
            ],
        )
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict))
        self.assertIn((self.parser.filename, 4, op2), pre_dict.content)
        self.assertIn((self.parser.filename, 5, filter2), pre_dict.content)
        self.assertEqual(
            self.parser.node.get_failed_cases(),
            [([], [(self.parser.filename, 5, filter2)], [])],
        )

    def test_get_dicts_plain_deep(self):
        self.parser.parse_string("""
            k1 = v0
            k2 = v0
            ka = v0
            kb = v0
            variants:
                - test1:
                    k1 = v1
                - test2:
                    k2 = v2
            variants:
                - a:
                    ka = va
                - b:
                    kb = vb
        """)
        pre_dict = parser.PreDict()

        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "a.test1")
        self.assertEqual(d["_name_map_file"]["<string>"], "a.test1")
        self.assertEqual(d["ka"], "va")
        self.assertEqual(d["kb"], "v0")
        self.assertEqual(d["k1"], "v1")
        self.assertEqual(d["k2"], "v0")
        self.assertEqual(pre_dict.route, [0, 0, 0, 0, None])

        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "a.test2")
        self.assertEqual(d["_name_map_file"]["<string>"], "a.test2")
        self.assertEqual(d["ka"], "va")
        self.assertEqual(d["kb"], "v0")
        self.assertEqual(d["k1"], "v0")
        self.assertEqual(d["k2"], "v2")
        self.assertEqual(pre_dict.route, [0, 0, 1, 0, None])

        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "b.test1")
        self.assertEqual(d["_name_map_file"]["<string>"], "b.test1")
        self.assertEqual(d["ka"], "v0")
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["k1"], "v1")
        self.assertEqual(d["k2"], "v0")
        self.assertEqual(pre_dict.route, [1, 0, 0, 0, None])

        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "b.test2")
        self.assertEqual(d["_name_map_file"]["<string>"], "b.test2")
        self.assertEqual(d["ka"], "v0")
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["k1"], "v0")
        self.assertEqual(d["k2"], "v2")
        self.assertEqual(pre_dict.route, [1, 0, 1, 0, None])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict))

        # no more dictionaries
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict))
        self.assertEqual(pre_dict.route, [2])

    def test_get_dicts_plain_deep_filtered(self):
        self.parser.parse_string("""
            variants:
                - test1:
                    k1 = v1
                - test2:
                    k2 = v2
                    no b
            variants:
                - a:
                    ka = va
                    only test2
                - b:
                    kb = vb
        """)
        pre_dict = parser.PreDict()

        # skip a.test1
        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "a.test2")
        self.assertEqual(d["ka"], "va")
        self.assertEqual(d["k2"], "v2")
        self.assertEqual(pre_dict.route, [0, 0, 1, 0, None])

        # skip b.test2
        d = self.parser.get_dicts_plain(pre_dict)
        self.assertEqual(d["name"], "b.test1")
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["k1"], "v1")
        self.assertEqual(pre_dict.route, [1, 0, 0, 0, None])
        self.assertIsNone(self.parser.get_dicts_plain(pre_dict))
        self.assertEqual(pre_dict.route, [2])

    def test_join_filters(self):
        self.parser.parse_string("variants:\n  - test1:\n    key1 = value1\n  - test2:\n    key2 = value2\n")
        self.parser.filename = "testfile"
        joins = [(self.parser.filename, 1, parser.OnlyFilter([[[parser.Label("test1")]]], "test1")),
                 (self.parser.filename, 1, parser.OnlyFilter([[[parser.Label("test2")]]], "test2"))]
        pre_dict = parser.PreDict()
        pre_dict.update_from_node(self.parser.node)
        pre_dict.joins[-1] = joins
        pre_dict.join_dicts[-1] = [None for _ in joins]
        pre_dict.join_pre_dicts[-1] = [None for _ in joins]

        d = self.parser.join_filters(pre_dict)
        self.assertEqual(d["name"], "test1.test2")
        self.assertEqual(d["key1"], "value1")
        self.assertEqual(d["key2"], "value2")
        self.assertEqual(pre_dict.route, [None])
        self.assertEqual(pre_dict.joins, [joins])
        self.assertEqual(len(pre_dict.join_dicts), 1)
        self.assertEqual(len(pre_dict.join_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_dicts[-1][0]["name"], "test1")
        self.assertEqual(pre_dict.join_dicts[-1][1]["name"], "test2")
        self.assertEqual(len(pre_dict.join_pre_dicts), 1)
        self.assertEqual(len(pre_dict.join_pre_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].content[0], joins[0])
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].content[0], joins[1])

    def test_join_filters_deep(self):
        self.parser.parse_string("""
            k1 = v0
            k2 = v0
            ka = v0
            kb = v0
            variants:
                - test1:
                    k1 = v1
                - test2:
                    k2 = v2
            variants:
                - a:
                    ka = va
                - b:
                    kb = vb
        """)
        self.parser.filename = "testfile"
        joins = [(self.parser.filename, 1, parser.OnlyFilter([[[parser.Label("a")]]], "test1")),
                 (self.parser.filename, 1, parser.OnlyFilter([[[parser.Label("b")]]], "test2"))]
        pre_dict = parser.PreDict()
        pre_dict.update_from_node(self.parser.node)
        pre_dict.joins[-1] = joins
        pre_dict.join_dicts[-1] = [None for _ in joins]
        pre_dict.join_pre_dicts[-1] = [None for _ in joins]

        d = self.parser.join_filters(pre_dict)
        self.assertEqual(d["name"], "a.test1.b.test1")
        # b variant contains overwriting default value for ka
        self.assertEqual(d["ka"], "v0")
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["k1"], "v1")
        self.assertEqual(d["k2"], "v0")
        self.assertEqual(pre_dict.route, [None])
        self.assertEqual(pre_dict.joins, [joins])
        self.assertEqual(len(pre_dict.join_dicts), 1)
        self.assertEqual(len(pre_dict.join_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_dicts[-1][0]["name"], "a.test1")
        self.assertEqual(pre_dict.join_dicts[-1][1]["name"], "b.test1")
        self.assertEqual(len(pre_dict.join_pre_dicts), 1)
        self.assertEqual(len(pre_dict.join_pre_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].content[0], joins[0])
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].content[0], joins[1])

        d = self.parser.join_filters(pre_dict)
        self.assertEqual(d["name"], "a.test1.b.test2")
        # b variant contains overwriting default value for ka
        self.assertEqual(d["ka"], "v0")
        self.assertEqual(d["kb"], "vb")
        # b.test2 variant contains overwriting default value for k1
        self.assertEqual(d["k1"], "v0")
        self.assertEqual(d["k2"], "v2")
        self.assertEqual(pre_dict.route, [None])
        self.assertEqual(pre_dict.joins, [joins])
        self.assertEqual(len(pre_dict.join_dicts), 1)
        self.assertEqual(len(pre_dict.join_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_dicts[-1][0]["name"], "a.test1")
        self.assertEqual(pre_dict.join_dicts[-1][1]["name"], "b.test2")
        self.assertEqual(len(pre_dict.join_pre_dicts), 1)
        self.assertEqual(len(pre_dict.join_pre_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].content[0], joins[0])
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].content[0], joins[1])

        d = self.parser.join_filters(pre_dict)
        self.assertEqual(d["name"], "a.test2.b.test1")
        # b variant contains overwriting default value for ka
        self.assertEqual(d["ka"], "v0")
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["k1"], "v1")
        # b.test1 variant contains overwriting default value for k2
        self.assertEqual(d["k2"], "v0")
        self.assertEqual(pre_dict.route, [None])
        self.assertEqual(pre_dict.joins, [joins])
        self.assertEqual(len(pre_dict.join_dicts), 1)
        self.assertEqual(len(pre_dict.join_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_dicts[-1][0]["name"], "a.test2")
        self.assertEqual(pre_dict.join_dicts[-1][1]["name"], "b.test1")
        self.assertEqual(len(pre_dict.join_pre_dicts), 1)
        self.assertEqual(len(pre_dict.join_pre_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].content[0], joins[0])
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].content[0], joins[1])

        d = self.parser.join_filters(pre_dict)
        self.assertEqual(d["name"], "a.test2.b.test2")
        # b variant contains overwriting default value for ka
        self.assertEqual(d["ka"], "v0")
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["k1"], "v0")
        self.assertEqual(d["k2"], "v2")
        self.assertEqual(pre_dict.route, [None])
        self.assertEqual(pre_dict.joins, [joins])
        self.assertEqual(len(pre_dict.join_dicts), 1)
        self.assertEqual(len(pre_dict.join_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_dicts[-1][0]["name"], "a.test2")
        self.assertEqual(pre_dict.join_dicts[-1][1]["name"], "b.test2")
        self.assertEqual(len(pre_dict.join_pre_dicts), 1)
        self.assertEqual(len(pre_dict.join_pre_dicts[-1]), 2)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][0].content[0], joins[0])
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].branch[0], self.parser.node)
        self.assertEqual(pre_dict.join_pre_dicts[-1][1].content[0], joins[1])

    def test_get_dicts_joined(self):
        self.parser.parse_string("""
            k1 = v0
            k2 = v0
            ka = v0
            kb = v0
            variants:
                - test1:
                    k1 = v1
                    suffix _s1
                - test2:
                    k2 = v2
                    suffix _s2
            variants:
                - a:
                    ka = va
                    join test1 test2
                - b:
                    kb = vb
                    join test1 test2
        """)
        pre_dict = parser.PreDict()

        d = self.parser.get_dicts_joined(pre_dict)
        self.assertEqual(d["name"], "a.test1.test2")
        # variant local parameter is overwritten
        self.assertEqual(d["ka"], "va")
        self.assertEqual(d["ka_s1"], "v0")
        self.assertEqual(d["ka_s2"], "v0")
        # variant default parameter is not overwritten
        self.assertEqual(d["kb"], "v0")
        self.assertNotIn("kb_s1", d)
        self.assertNotIn("kb_s2", d)
        # joined default and overwritten parameters
        self.assertNotIn("k1", d)
        self.assertEqual(d["k1_s1"], "v1")
        self.assertEqual(d["k1_s2"], "v0")
        self.assertNotIn("k2", d)
        self.assertEqual(d["k2_s1"], "v0")
        self.assertEqual(d["k2_s2"], "v2")
        self.assertEqual(pre_dict.route, [0, None])
        self.assertEqual(len(pre_dict.joins), 2)
        self.assertIsNone(pre_dict.joins[0])
        self.assertIsNotNone(pre_dict.joins[1])

        self.parser.parent_generator = True
        d = self.parser.get_dicts_joined(pre_dict)
        self.assertEqual(d["name"], "b.test1.test2")
        # variant default parameter is not overwritten
        self.assertEqual(d["ka"], "v0")
        self.assertNotIn("ka_s1", d)
        self.assertNotIn("ka_s2", d)
        # variant local parameter is overwritten
        self.assertEqual(d["kb"], "vb")
        self.assertEqual(d["kb_s1"], "v0")
        self.assertEqual(d["kb_s2"], "v0")
        # joined default and overwritten parameters
        self.assertNotIn("k1", d)
        self.assertEqual(d["k1_s1"], "v1")
        self.assertEqual(d["k1_s2"], "v0")
        self.assertNotIn("k2", d)
        self.assertEqual(d["k2_s1"], "v0")
        self.assertEqual(d["k2_s2"], "v2")
        self.assertEqual(pre_dict.route, [1, None])
        self.assertEqual(len(pre_dict.joins), 2)
        self.assertIsNone(pre_dict.joins[0])
        self.assertIsNotNone(pre_dict.joins[1])

        self.assertIsNone(self.parser.get_dicts_joined(pre_dict))
        self.assertEqual(pre_dict.route, [2])
        self.assertEqual(len(pre_dict.joins), 1)
        self.assertIsNone(pre_dict.joins[0])

    def test_get_dicts_joined_single(self):
        self.parser.parse_string("variants:\n  - test:\n    key = value\n    join test\n")
        d = self.parser.get_dicts_joined()
        self.assertEqual(d["name"], "test")
        self.assertEqual(d["key"], "value")

    def _compare_parser_dictionaries(self, parser: parser.Parser, reference: dict[str, str]) -> None:
        """Check if the parser dictionaries match reference ones."""
        result = list(parser.get_dicts())
        # as the dictionary list is very large, test each item individually:
        self.assertEqual(len(result), len(reference))
        for resdict, refdict in zip(result, reference):
            # checking the dict name first should make some errors more visible
            self.assertEqual(resdict.get('name'), refdict.get('name'))
            self.assertEqual(resdict, refdict)

    def _compare_config_dump(self, config: str, dump: str) -> None:
        """Check if the parsed dictionaries from a config match dumped ones."""
        configpath = os.path.join(testdatadir, config)
        dumppath = os.path.join(testdatadir, dump)

        with gzip.open(dumppath, 'rt') if dumppath.endswith('.gz') else open(dumppath, 'r') as df:
            # we could have used pickle, but repr()-based dumps are easier to
            # generate, debug, and edit
            dumpdata = eval(df.read())

        p = parser.Parser(configpath)
        self._compare_parser_dictionaries(p, dumpdata)

    def _compare_string_config(self, string: str, reference: dict[str, str], defaults: bool = False) -> None:
        """Check if the parsed dictionaries from a string match reference ones."""
        p = parser.Parser(defaults=defaults)
        p.parse_string(string)
        self._compare_parser_dictionaries(p, reference)

    def test_variants_simple(self):
        self._compare_string_config("""
            c = abc
            x = vc
            variants:
                - a:
                    x = va
                - b:
                    x = vb
            """,
            [
                {'_name_map_file': {'<string>': 'a'},
                 '_short_name_map_file': {'<string>': 'a'},
                 'c': 'abc',
                 'dep': [],
                 'name': 'a',
                 'shortname': 'a',
                 'x': 'va'},
                {'_name_map_file': {'<string>': 'b'},
                 '_short_name_map_file': {'<string>': 'b'},
                 'c': 'abc',
                 'dep': [],
                 'name': 'b',
                 'shortname': 'b',
                 'x': 'vb'},
            ])

    def test_variants_product(self):
        self._compare_string_config("""
            c = abc
            x = vc
            y = w3
            variants:
                - a:
                    x = va
                - b:
                    x = vb
            variants:
                - 1:
                    y = w1
                - 2:
                    y = w2
            """,
            [
                {'_name_map_file': {'<string>': '1.a'},
                 '_short_name_map_file': {'<string>': '1.a'},
                 'c': 'abc',
                 'dep': [],
                 'name': '1.a',
                 'shortname': '1.a',
                 'x': 'va',
                 'y': 'w1'},
                {'_name_map_file': {'<string>': '1.b'},
                 '_short_name_map_file': {'<string>': '1.b'},
                'c': 'abc',
                 'dep': [],
                 'name': '1.b',
                 'shortname': '1.b',
                 'x': 'vb',
                 'y': 'w1'},
                {'_name_map_file': {'<string>': '2.a'},
                 '_short_name_map_file': {'<string>': '2.a'},
                 'c': 'abc',
                 'dep': [],
                 'name': '2.a',
                 'shortname': '2.a',
                 'x': 'va',
                 'y': 'w2'},
                {'_name_map_file': {'<string>': '2.b'},
                '_short_name_map_file': {'<string>': '2.b'},
                 'c': 'abc',
                 'dep': [],
                 'name': '2.b',
                 'shortname': '2.b',
                 'x': 'vb',
                 'y': 'w2'},
            ])

    def test_filter_mixing(self):
        self._compare_string_config("""
            variants:
                - unknown_qemu:
                - rhel64:
            only unknown_qemu
            variants:
                - kvm:
                - nokvm:
            variants:
                - testA:
                    nokvm:
                        no unknown_qemu
                - testB:
            """,
            [
                {'_name_map_file': {'<string>': 'testA.kvm.unknown_qemu'},
                 '_short_name_map_file': {'<string>': 'testA.kvm.unknown_qemu'},
                 'dep': [],
                 'name': 'testA.kvm.unknown_qemu',
                 'shortname': 'testA.kvm.unknown_qemu'},
                {'_name_map_file': {'<string>': 'testB.kvm.unknown_qemu'},
                 '_short_name_map_file': {'<string>': 'testB.kvm.unknown_qemu'},
                 'dep': [],
                 'name': 'testB.kvm.unknown_qemu',
                 'shortname': 'testB.kvm.unknown_qemu'},
                {'_name_map_file': {'<string>': 'testB.nokvm.unknown_qemu'},
                 '_short_name_map_file': {'<string>': 'testB.nokvm.unknown_qemu'},
                 'dep': [],
                 'name': 'testB.nokvm.unknown_qemu',
                 'shortname': 'testB.nokvm.unknown_qemu'},
            ])

    def test_named_variants(self):
        self._compare_string_config("""
            variants tests: # All tests in configuration
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"

            variants virt_system:
              - @linux:
              - windows:

            variants host_os:
              - linux:
                   image = linux
              - windows:
                   image = windows

            only (host_os=linux)
            """,
            [
                {'_name_map_file': {'<string>': '(host_os=linux).(virt_system=linux).(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'linux.linux.wait.long'},
                 'dep': [],
                 'host_os': 'linux',
                 'image': 'linux',
                 'name': '(host_os=linux).(virt_system=linux).(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'linux.wait.long',
                 'tests': 'wait',
                 'time': 'short_time',
                 'virt_system': 'linux'},
                {'_name_map_file': {'<string>': '(host_os=linux).(virt_system=linux).(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'linux.linux.wait.short'},
                 'dep': ['(host_os=linux).(virt_system=linux).(tests=wait).long'],
                 'host_os': 'linux',
                 'image': 'linux',
                 'name': '(host_os=linux).(virt_system=linux).(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'linux.wait.short',
                 'tests': 'wait',
                 'time': 'long_time',
                 'virt_system': 'linux'},
                {'_name_map_file': {'<string>': '(host_os=linux).(virt_system=linux).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'linux.linux.test2'},
                 'dep': [],
                 'host_os': 'linux',
                 'image': 'linux',
                 'name': '(host_os=linux).(virt_system=linux).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'linux.test2',
                 'tests': 'test2',
                 'virt_system': 'linux'},
                {'_name_map_file': {'<string>': '(host_os=linux).(virt_system=windows).(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'linux.windows.wait.long'},
                 'dep': [],
                 'host_os': 'linux',
                 'image': 'linux',
                 'name': '(host_os=linux).(virt_system=windows).(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'linux.windows.wait.long',
                 'tests': 'wait',
                 'time': 'short_time',
                 'virt_system': 'windows'},
                {'_name_map_file': {'<string>': '(host_os=linux).(virt_system=windows).(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'linux.windows.wait.short'},
                 'dep': ['(host_os=linux).(virt_system=windows).(tests=wait).long'],
                 'host_os': 'linux',
                 'image': 'linux',
                 'name': '(host_os=linux).(virt_system=windows).(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'linux.windows.wait.short',
                 'tests': 'wait',
                 'time': 'long_time',
                 'virt_system': 'windows'},
                {'_name_map_file': {'<string>': '(host_os=linux).(virt_system=windows).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'linux.windows.test2'},
                 'dep': [],
                 'host_os': 'linux',
                 'image': 'linux',
                 'name': '(host_os=linux).(virt_system=windows).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'linux.windows.test2',
                 'tests': 'test2',
                 'virt_system': 'windows'},
            ])

    def test_variant_defaults(self):
        self._compare_string_config("""
            variants tests:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"

            variants virt_system [ default=linux ]:
              - linux:
              - @windows:

            variants host_os:
              - linux:
                   image = linux
              - @windows:
                   image = windows
            """,
            [
                {'_name_map_file': {'<string>': '(host_os=windows).(virt_system=linux).(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'windows.linux.wait.long'},
                 'dep': [],
                 'host_os': 'windows',
                 'image': 'windows',
                 'name': '(host_os=windows).(virt_system=linux).(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'wait.long',
                 'tests': 'wait',
                 'time': 'short_time',
                 'virt_system': 'linux'},
                {'_name_map_file': {'<string>': '(host_os=windows).(virt_system=linux).(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'windows.linux.wait.short'},
                 'dep': ['(host_os=windows).(virt_system=linux).(tests=wait).long'],
                 'host_os': 'windows',
                 'image': 'windows',
                 'name': '(host_os=windows).(virt_system=linux).(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'wait.short',
                 'tests': 'wait',
                 'time': 'long_time',
                 'virt_system': 'linux'},
                {'_name_map_file': {'<string>': '(host_os=windows).(virt_system=linux).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'windows.linux.test2'},
                 'dep': [],
                 'host_os': 'windows',
                 'image': 'windows',
                 'name': '(host_os=windows).(virt_system=linux).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'test2',
                 'tests': 'test2',
                 'virt_system': 'linux'},
            ],
            True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                variants tests [default=system2]:
                  - system1:
                """,
                          [],
                          True)

    def test_del(self):
        self._compare_string_config("""
            variants tests:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"
            """,
            [
                {'_name_map_file': {'<string>': '(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'wait.long'},
                 'dep': [],
                 'name': '(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'wait.long',
                 'tests': 'wait',
                 'time': 'short_time'},
                {'_name_map_file': {'<string>': '(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'wait.short'},
                 'dep': ['(tests=wait).long'],
                 'name': '(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'wait.short',
                 'tests': 'wait',
                 'time': 'long_time'},
                {'_name_map_file': {'<string>': '(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'test2'},
                 'dep': [],
                 'name': '(tests=test2)',
                 'run': 'test1',
                 'shortname': 'test2',
                 'tests': 'test2'},
            ],
            True)

        self._compare_string_config("""
            variants tests:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"

            del time
            """,
            [
                {'_name_map_file': {'<string>': '(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'wait.long'},
                 'dep': [],
                 'name': '(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'wait.long',
                 'tests': 'wait'},
                {'_name_map_file': {'<string>': '(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'wait.short'},
                 'dep': ['(tests=wait).long'],
                 'name': '(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'wait.short',
                 'tests': 'wait'},
                {'_name_map_file': {'<string>': '(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'test2'},
                 'dep': [],
                 'name': '(tests=test2)',
                 'run': 'test1',
                 'shortname': 'test2',
                 'tests': 'test2'},
            ],
            True)

    def test_suffix_join_del(self):
        self._compare_string_config("""
            variants:
                - x:
                  foo = x
                  suffix _x
                - y:
                  foo = y
                  suffix _y
                - z:
                  foo = z
            variants:
                - control_group:
                - del_raw:
                    del foo
                - del_suffix:
                    del foo_x
                - control_group_xy:
                    join x y
                - del_raw_xy:
                    join x y
                    del foo
                # TODO: the regex matching for the del operator does not work
                #- del_regex:
                #    del foo(_.*)?
                - del_suffix_xy:
                    join x y
                    del foo_x
                - control_group_xz:
                    join x z
                - del_raw_xz:
                    join x z
                    del foo
                - del_suffix_xz:
                    join x z
                    del foo_x
            """,
            [
                {'_name_map_file': {'<string>': 'control_group.x'},
                 '_short_name_map_file': {'<string>': 'control_group.x'},
                 'dep': [],
                 'name': 'control_group.x',
                 'shortname': 'control_group.x',
                 'foo': 'x'},
                {'_name_map_file': {'<string>': 'control_group.y'},
                 '_short_name_map_file': {'<string>': 'control_group.y'},
                 'dep': [],
                 'name': 'control_group.y',
                 'shortname': 'control_group.y',
                 'foo': 'y'},
                {'_name_map_file': {'<string>': 'control_group.z'},
                 '_short_name_map_file': {'<string>': 'control_group.z'},
                 'dep': [],
                 'name': 'control_group.z',
                 'shortname': 'control_group.z',
                 'foo': 'z'},
                {'_name_map_file': {'<string>': 'del_raw.x'},
                 '_short_name_map_file': {'<string>': 'del_raw.x'},
                 'dep': [],
                 'name': 'del_raw.x',
                 'shortname': 'del_raw.x',
                 'foo': 'x'},
                {'_name_map_file': {'<string>': 'del_raw.y'},
                 '_short_name_map_file': {'<string>': 'del_raw.y'},
                 'dep': [],
                 'name': 'del_raw.y',
                 'shortname': 'del_raw.y',
                 'foo': 'y'},
                {'_name_map_file': {'<string>': 'del_raw.z'},
                 '_short_name_map_file': {'<string>': 'del_raw.z'},
                 'dep': [],
                 'name': 'del_raw.z',
                 'shortname': 'del_raw.z'},
                {'_name_map_file': {'<string>': 'del_suffix.x'},
                 '_short_name_map_file': {'<string>': 'del_suffix.x'},
                 'dep': [],
                 'name': 'del_suffix.x',
                 'shortname': 'del_suffix.x'},
                {'_name_map_file': {'<string>': 'del_suffix.y'},
                 '_short_name_map_file': {'<string>': 'del_suffix.y'},
                 'dep': [],
                 'name': 'del_suffix.y',
                 'shortname': 'del_suffix.y',
                 'foo': 'y'},
                {'_name_map_file': {'<string>': 'del_suffix.z'},
                 '_short_name_map_file': {'<string>': 'del_suffix.z'},
                 'dep': [],
                 'name': 'del_suffix.z',
                 'shortname': 'del_suffix.z',
                 'foo': 'z'},
                {'_name_map_file': {'<string>': 'control_group_xy.y'},
                 '_short_name_map_file': {'<string>': 'control_group_xy.y'},
                 'dep': [],
                 'name': 'control_group_xy.x.y',
                 'shortname': 'control_group_xy.x.y',
                 'foo_x': 'x',
                 'foo_y': 'y'},
                {'_name_map_file': {'<string>': 'del_raw_xy.y'},
                 '_short_name_map_file': {'<string>': 'del_raw_xy.y'},
                 'dep': [],
                 'name': 'del_raw_xy.x.y',
                 'shortname': 'del_raw_xy.x.y',
                 'foo_x': 'x',
                 'foo_y': 'y'},
                {'_name_map_file': {'<string>': 'del_suffix_xy.y'},
                 '_short_name_map_file': {'<string>': 'del_suffix_xy.y'},
                 'dep': [],
                 'name': 'del_suffix_xy.x.y',
                 'shortname': 'del_suffix_xy.x.y',
                 'foo': 'y'},
                {'_name_map_file': {'<string>': 'control_group_xz.z'},
                 '_short_name_map_file': {'<string>': 'control_group_xz.z'},
                 'dep': [],
                 'name': 'control_group_xz.x.z',
                 'shortname': 'control_group_xz.x.z',
                 'foo': 'z',
                 'foo_x': 'x'},
                {'_name_map_file': {'<string>': 'del_raw_xz.z'},
                 '_short_name_map_file': {'<string>': 'del_raw_xz.z'},
                 'dep': [],
                 'name': 'del_raw_xz.x.z',
                 'shortname': 'del_raw_xz.x.z',
                 'foo': 'x'},
                {'_name_map_file': {'<string>': 'del_suffix_xz.z'},
                 '_short_name_map_file': {'<string>': 'del_suffix_xz.z'},
                 'dep': [],
                 'name': 'del_suffix_xz.x.z',
                 'shortname': 'del_suffix_xz.x.z',
                 'foo': 'z'},
            ],
            True)

        self._compare_string_config("""
            variants tests:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"

            del time
            """,
            [
                {'_name_map_file': {'<string>': '(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'wait.long'},
                 'dep': [],
                 'name': '(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'wait.long',
                 'tests': 'wait'},
                {'_name_map_file': {'<string>': '(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'wait.short'},
                 'dep': ['(tests=wait).long'],
                 'name': '(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'wait.short',
                 'tests': 'wait'},
                {'_name_map_file': {'<string>': '(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'test2'},
                 'dep': [],
                 'name': '(tests=test2)',
                 'run': 'test1',
                 'shortname': 'test2',
                 'tests': 'test2'},
            ],
            True)

    def test_missing_include(self):
        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                include xxxxxxxxx/xxxxxxxxxxx
                """,
                          [],
                          True)

    def test_variable_assignment(self):
        self._compare_string_config("""
            variants tests:
              -system1:
                    var = 1
                    var = 2
                    var += a
                    var <= b
                    system = 2
                    variable-name-with-dashes = sampletext
                    ddd = tests variant is ${tests}
                    dashes = show ${variable-name-with-dashes}
                    error = ${tests + str(int(system) + 3)}4
                    s.* ?= ${tests}ahoj4
                    s.* ?+= c
                    s.* ?<= d
                    s(_.*)? ?= b1
                    s(_.*)? ?= b2(c=d)
                    system2.allowed: s = t
                    system += 4
                    var += "test"
                    1st = 1
                    starts_with_number = index ${1st}
                    not_a_substitution = ${}
            """,
            [
                {'_name_map_file': {'<string>': '(tests=system1)'},
                 '_short_name_map_file': {'<string>': 'system1'},
                 'variable-name-with-dashes': 'sampletext',
                 'ddd': 'tests variant is system1',
                 'dashes': 'show sampletext',
                 'dep': [],
                 'error': '${tests + str(int(system) + 3)}4',
                 'name': '(tests=system1)',
                 'shortname': 'system1',
                 'system': 'dsystem1ahoj4c4',
                 'tests': 'system1',
                 'var': 'b2atest',
                 '1st': '1',
                 'starts_with_number': 'index 1',
                 'not_a_substitution': '${}',
                 },
            ],
            True)

    def test_variable_lazy_assignment(self):
        self._compare_string_config("""
            arg1 = ~balabala
            variants:
                - base_content:
                    foo = bar
                - empty_content:
            variants:
                - lazy_set:
                    foo ~= baz
                - lazy_set_with_substitution:
                    foo ~= ${arg1}
                - lazy_set_with_double_token:
                    foo ~= ~= foo
                - dummy_set:
            foo ~= qux
            """,
            [
                {'_name_map_file': {'<string>': 'lazy_set.base_content'},
                 '_short_name_map_file': {'<string>': 'lazy_set.base_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': 'bar',
                 'name': 'lazy_set.base_content',
                 'shortname': 'lazy_set.base_content'},
                {'_name_map_file': {'<string>': 'lazy_set.empty_content'},
                 '_short_name_map_file': {'<string>': 'lazy_set.empty_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': 'baz',
                 'name': 'lazy_set.empty_content',
                 'shortname': 'lazy_set.empty_content'},
                {'_name_map_file': {'<string>': 'lazy_set_with_substitution.base_content'},
                 '_short_name_map_file': {'<string>': 'lazy_set_with_substitution.base_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': 'bar',
                 'name': 'lazy_set_with_substitution.base_content',
                 'shortname': 'lazy_set_with_substitution.base_content'},
                {'_name_map_file': {'<string>': 'lazy_set_with_substitution.empty_content'},
                 '_short_name_map_file': {'<string>': 'lazy_set_with_substitution.empty_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': '~balabala',
                 'name': 'lazy_set_with_substitution.empty_content',
                 'shortname': 'lazy_set_with_substitution.empty_content'},
                {'_name_map_file': {'<string>': 'lazy_set_with_double_token.base_content'},
                 '_short_name_map_file': {'<string>': 'lazy_set_with_double_token.base_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': 'bar',
                 'name': 'lazy_set_with_double_token.base_content',
                 'shortname': 'lazy_set_with_double_token.base_content'},
                {'_name_map_file': {'<string>': 'lazy_set_with_double_token.empty_content'},
                 '_short_name_map_file': {'<string>': 'lazy_set_with_double_token.empty_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': '~= foo',
                 'name': 'lazy_set_with_double_token.empty_content',
                 'shortname': 'lazy_set_with_double_token.empty_content'},
                {'_name_map_file': {'<string>': 'dummy_set.base_content'},
                 '_short_name_map_file': {'<string>': 'dummy_set.base_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': 'bar',
                 'name': 'dummy_set.base_content',
                 'shortname': 'dummy_set.base_content'},
                {'_name_map_file': {'<string>': 'dummy_set.empty_content'},
                 '_short_name_map_file': {'<string>': 'dummy_set.empty_content'},
                 'arg1': '~balabala',
                 'dep': [],
                 'foo': 'qux',
                 'name': 'dummy_set.empty_content',
                 'shortname': 'dummy_set.empty_content'},
            ],
            True)

    def test_condition(self):
        self._compare_string_config("""
            variants tests [meta1]:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"

            test2: bbb = aaaa
               aaa = 1
            """,
            [
                {'_name_map_file': {'<string>': '(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'wait.long'},
                 'dep': [],
                 'name': '(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'wait.long',
                 'tests': 'wait',
                 'time': 'short_time'},
                {'_name_map_file': {'<string>': '(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'wait.short'},
                 'dep': ['(tests=wait).long'],
                 'name': '(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'wait.short',
                 'tests': 'wait',
                 'time': 'long_time'},
                {'_name_map_file': {'<string>': '(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'test2'},
                 'aaa': '1',
                 'bbb': 'aaaa',
                 'dep': [],
                 'name': '(tests=test2)',
                 'run': 'test1',
                 'shortname': 'test2',
                 'tests': 'test2'},
            ],
            True)
        self._compare_string_config("""
            variants:
                - a:
                    foo = foo
                    c:
                        foo = bar
                - b:
                    foo = foob
            variants:
                - c:
                    bala = lalalala
                    a:
                       bala = balabala
                - d:
            """,
            [
                {'_name_map_file': {'<string>': 'c.a'},
                 '_short_name_map_file': {'<string>': 'c.a'},
                 'bala': 'balabala',
                 'dep': [],
                 'foo': 'bar',
                 'name': 'c.a',
                 'shortname': 'c.a'},
                {'_name_map_file': {'<string>': 'c.b'},
                 '_short_name_map_file': {'<string>': 'c.b'},
                 'bala': 'lalalala',
                 'dep': [],
                 'foo': 'foob',
                 'name': 'c.b',
                 'shortname': 'c.b'},
                {'_name_map_file': {'<string>': 'd.a'},
                 '_short_name_map_file': {'<string>': 'd.a'},
                 'dep': [],
                 'foo': 'foo',
                 'name': 'd.a',
                 'shortname': 'd.a'},
                {'_name_map_file': {'<string>': 'd.b'},
                 '_short_name_map_file': {'<string>': 'd.b'},
                 'dep': [],
                 'foo': 'foob',
                 'name': 'd.b',
                 'shortname': 'd.b'},
            ],
            True)

    def test_negative_condition(self):
        self._compare_string_config("""
            variants tests [meta1]:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
              - test2:
                   run = "test1"

            !test2: bbb = aaaa
               aaa = 1
            """,
            [
                {'_name_map_file': {'<string>': '(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'wait.long'},
                 'aaa': '1',
                 'bbb': 'aaaa',
                 'dep': [],
                 'name': '(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'wait.long',
                 'tests': 'wait',
                 'time': 'short_time'},
                {'_name_map_file': {'<string>': '(tests=wait).short'},
                 '_short_name_map_file': {'<string>': 'wait.short'},
                 'aaa': '1',
                 'bbb': 'aaaa',
                 'dep': ['(tests=wait).long'],
                 'name': '(tests=wait).short',
                 'run': 'wait',
                 'shortname': 'wait.short',
                 'tests': 'wait',
                 'time': 'long_time'},
                {'_name_map_file': {'<string>': '(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'test2'},
                 'dep': [],
                 'name': '(tests=test2)',
                 'run': 'test1',
                 'shortname': 'test2',
                 'tests': 'test2'},
            ],
            True)

    def test_special_chars(self):
        self._compare_string_config("""
            FILE_2018年度公开课计划表.xlsx = type no_txt, desc (check that unicode filename is handled)
            """,
            [
                {'dep': [],
                 'name': '',
                 'shortname': '',
                 'FILE_2018年度公开课计划表.xlsx': 'type no_txt, desc (check that unicode filename is handled)',
                 },
            ],
            True)

    def test_syntax_errors(self):
        self.assertRaises(parser.LexerError,
                          self._compare_string_config, """
                variants tests$:
                  - system1:
                        var = 1
                        var = 2
                        var += a
                        var <= b
                        system = 2
                        s.* ?= ${tests}4
                        s.* ?+= c
                        s.* ?<= d
                        system += 4
                """,
                          [],
                          True)

        self.assertRaises(parser.LexerError,
                          self._compare_string_config, """
                variants tests [defaul$$$$t=system1]:
                  - system1:
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                variants tests [default=system1] wrong:
                  - system1:
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                only xxx...yyy
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                only xxx..,yyy
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                aaabbbb.ddd
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                aaa.bbb:
                  variants test:
                     -sss:
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                variants test [sss = bbb:
                     -sss:
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                variants test [default]:
                     -sss:
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                variants test [default] ddd:
                     -sss:
                """,
                          [],
                          True)

        self.assertRaises(parser.ParserError,
                          self._compare_string_config, """
                variants test [default] ddd
                """,
                          [],
                          True)

        self.assertRaises(parser.LexerError,
                          self._compare_string_config, """
                variants tests:
                  wait:
                       run = "wait"
                       variants:
                         - long:
                            time = short_time
                         - short: long
                            time = long_time
                  - test2:
                       run = "test1"
                """,
                          [],
                          True)

    def test_complicated_filter(self):
        self._compare_string_config("""
            variants tests:
              - wait:
                   run = "wait"
                   variants:
                     - long:
                        time = short_time
                     - short: long
                        time = long_time
                        only (host_os=linux), ( guest_os =    linux  )
              - test2:
                   run = "test1"

            variants guest_os:
              - linux:
                    install = linux
                    no (tests=wait)..short
              - windows:
                    install = windows
                    only test2

            variants host_os:
              - linux:
                    start = linux
              - windows:
                    start = windows
                    only test2
            """,
            [
                {'_name_map_file': {'<string>': '(host_os=linux).(guest_os=linux).(tests=wait).long'},
                 '_short_name_map_file': {'<string>': 'linux.linux.wait.long'},
                 'dep': [],
                 'guest_os': 'linux',
                 'host_os': 'linux',
                 'install': 'linux',
                 'name': '(host_os=linux).(guest_os=linux).(tests=wait).long',
                 'run': 'wait',
                 'shortname': 'linux.linux.wait.long',
                 'start': 'linux',
                 'tests': 'wait',
                 'time': 'short_time'},
                {'_name_map_file': {'<string>': '(host_os=linux).(guest_os=linux).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'linux.linux.test2'},
                 'dep': [],
                 'guest_os': 'linux',
                 'host_os': 'linux',
                 'install': 'linux',
                 'name': '(host_os=linux).(guest_os=linux).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'linux.linux.test2',
                 'start': 'linux',
                 'tests': 'test2'},
                {'_name_map_file': {'<string>': '(host_os=linux).(guest_os=windows).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'linux.windows.test2'},
                 'dep': [],
                 'guest_os': 'windows',
                 'host_os': 'linux',
                 'install': 'windows',
                 'name': '(host_os=linux).(guest_os=windows).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'linux.windows.test2',
                 'start': 'linux',
                 'tests': 'test2'},
                {'_name_map_file': {'<string>': '(host_os=windows).(guest_os=linux).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'windows.linux.test2'},
                 'dep': [],
                 'guest_os': 'linux',
                 'host_os': 'windows',
                 'install': 'linux',
                 'name': '(host_os=windows).(guest_os=linux).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'windows.linux.test2',
                 'start': 'windows',
                 'tests': 'test2'},
                {'_name_map_file': {'<string>': '(host_os=windows).(guest_os=windows).(tests=test2)'},
                 '_short_name_map_file': {'<string>': 'windows.windows.test2'},
                 'dep': [],
                 'guest_os': 'windows',
                 'host_os': 'windows',
                 'install': 'windows',
                 'name': '(host_os=windows).(guest_os=windows).(tests=test2)',
                 'run': 'test1',
                 'shortname': 'windows.windows.test2',
                 'start': 'windows',
                 'tests': 'test2'},
            ],
            True)

    def test_join_substitution(self):
        self._compare_string_config("""
            key0 = "Baz"
            variants:
                - one:
                    key1 = "Hello"
                    key2 = "Foo"

                    test01 = "${key1}"
                    # the following substitutions are still not supported
                    #test02 = "${key1_v1}"
                    #test03 = "${key1_v2}"

                    suffix _v1
                - two:
                    key1 = "Bye"
                    key3 = "Bar"

                    test04 = "${key1}"
                    # the following substitutions are still not supported
                    #test05 = "${key1_v1}"
                    #test06 = "${key1_v2}"

                    suffix _v2
            variants:
                - alpha:
                    # the following substitutions are still not supported
                    #test07 = "${key1}"
                    #test08 = "${key1_v1}"
                    #test09 = "${key1_v2}"
                    #test10 = "${key2}"
                    #test11 = "${key3}"

                    key1 = "Alpha"
                    test12 = "${key1}"

                    join one two
                - beta:
                    # the following substitutions are still not supported
                    #test13 = "${key1}"
                    #test14 = "${key1_v1}"
                    #test15 = "${key1_v2}"
                    #test16 = "${key2}"
                    #test17 = "${key3}"

                    join one two

            test100 = "${key0}"
            # the following substitutions are still not supported
            #test18 = "${key1}"
            #test19 = "${key1_v1}"
            #test20 = "${key1_v2}"
            #test21 = "${key2}"
            #test22 = "${key3}"
            """,
            [
                {'_name_map_file': {'<string>': 'alpha.two'},
                 '_short_name_map_file': {'<string>': 'alpha.two'},
                 'dep': [],
                 'key0': 'Baz',
                 'key1': 'Alpha',
                 'key1_v1': 'Hello',
                 'key1_v2': 'Bye',
                 'key2': 'Foo',
                 'key3': 'Bar',
                 'name': 'alpha.one.two',
                 'shortname': 'alpha.one.two',
                 'test01': 'Hello',
                 #'test02': '${key1_v1}',
                 #'test03': '${key1_v2}',
                 'test04': 'Bye',
                 #'test05': '${key1_v1}',
                 #'test06': '${key1_v2}',
                 #'test07': 'Bye',
                 #'test08': '${key1_v1}',
                 #'test09': '${key1_v2}',
                 #'test10': '${key2}',
                 #'test11': 'Bar',
                 'test12': 'Alpha',
                 #'test18': 'Alpha',
                 #'test19': '${key1_v1}',
                 #'test20': 'Bye',
                 #'test21': '${key2}',
                 #'test22': 'Bar',
                 'test100': 'Baz'},
                {'_name_map_file': {'<string>': 'beta.two'},
                 '_short_name_map_file': {'<string>': 'beta.two'},
                 'dep': [],
                 'key0': 'Baz',
                 'key1_v1': 'Hello',
                 'key1_v2': 'Bye',
                 'key2': 'Foo',
                 'key3': 'Bar',
                 'name': 'beta.one.two',
                 'shortname': 'beta.one.two',
                 'test01': 'Hello',
                 #'test02': '${key1_v1}',
                 #'test03': '${key1_v2}',
                 'test04': 'Bye',
                 #'test05': '${key1_v1}',
                 #'test06': '${key1_v2}',
                 #'test13': 'Bye',
                 #'test14': '${key1_v1}',
                 #'test15': '${key1_v2}',
                 #'test16': '${key2}',
                 #'test17': 'Bar',
                 #'test18': 'Bye',
                 #'test19': '${key1_v1}',
                 #'test20': '${key1_v2}',
                 #'test21': '${key2}',
                 #'test22': 'Bar',
                 'test100': 'Baz'},
            ],
            True)

    def test_double_join(self):
        self._compare_string_config("""
            k1 = v0
            k2 = v0
            ka = v0
            kb = v0
            variants:
                - test1:
                    k1 = v1
                - test2:
                    k2 = v2
            variants:
                - a:
                    ka = va
                    suffix _s1
                - b:
                    kb = vb
                    suffix _s2
            join a b
        """, [
            {
                '_name_map_file': {'<string>': 'b.test1'},
                '_short_name_map_file': {'<string>': 'b.test1'},
                'dep': [],
                'name': 'a.test1.b.test1',
                'shortname': 'a.test1.b.test1',
                'ka_s1': 'va',
                'ka_s2': 'v0',
                'kb_s1': 'v0',
                'kb_s2': 'vb',
                'k1': 'v1',
                'k2': 'v0',
            },
            {
                '_name_map_file': {'<string>': 'b.test2'},
                '_short_name_map_file': {'<string>': 'b.test2'},
                'dep': [],
                'name': 'a.test1.b.test2',
                'shortname': 'a.test1.b.test2',
                'ka_s1': 'va',
                'ka_s2': 'v0',
                'kb_s1': 'v0',
                'kb_s2': 'vb',
                'k1_s1': 'v1',
                'k1_s2': 'v0',
                'k2_s1': 'v0',
                'k2_s2': 'v2',
            },
            {
                '_name_map_file': {'<string>': 'b.test1'},
                '_short_name_map_file': {'<string>': 'b.test1'},
                'dep': [],
                'name': 'a.test2.b.test1',
                'shortname': 'a.test2.b.test1',
                'ka_s1': 'va',
                'ka_s2': 'v0',
                'kb_s1': 'v0',
                'kb_s2': 'vb',
                'k1_s1': 'v0',
                'k1_s2': 'v1',
                'k2_s1': 'v2',
                'k2_s2': 'v0',
            },
            {
                '_name_map_file': {'<string>': 'b.test2'},
                '_short_name_map_file': {'<string>': 'b.test2'},
                'dep': [],
                'name': 'a.test2.b.test2',
                'shortname': 'a.test2.b.test2',
                'ka_s1': 'va',
                'ka_s2': 'v0',
                'kb_s1': 'v0',
                'kb_s2': 'vb',
                'k1': 'v0',
                'k2': 'v2',
            },
        ])


if __name__ == '__main__':
    unittest.main()
