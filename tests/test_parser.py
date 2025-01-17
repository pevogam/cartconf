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

        self.assertEqual(hash(label1), label1.hash_name())
        self.assertEqual(hash(label1), label1.hash_val)
        self.assertEqual(hash(label3), label3.hash_name())
        self.assertEqual(hash(label3), label3.hash_val)
        self.assertIsNone(label1.hash_var)
        self.assertEqual(label3.hash_var, label3.hash_variant())

        self.assertEqual(label1.hash_name(), label2.hash_name())
        self.assertEqual(label1.hash_variant(), label2.hash_variant())
        self.assertNotEqual(label1.hash_name(), label3.hash_name())
        self.assertNotEqual(label1.hash_variant(), label3.hash_variant())
        self.assertEqual(label3.hash_name(), label4.hash_name())
        self.assertEqual(label3.hash_variant(), label4.hash_variant())

        self.assertGreater(label3.hash_variant(), label3.hash_name())


class NodeTest(unittest.TestCase):

    def test_initialization(self):
        node = parser.Node()
        self.assertEqual(node.var_name, [])
        self.assertEqual(node.name, [])
        self.assertEqual(node.filename, "")
        self.assertEqual(node.dep, [])
        self.assertEqual(node.content, [])
        self.assertEqual(node.children, [])
        self.assertEqual(node.labels, set())
        self.assertFalse(node.append_to_shortname)
        self.assertEqual(node.failed_cases, collections.deque())
        self.assertFalse(node.default)

    def test_dump(self):
        node = parser.Node()
        empty_dumped_str = node.dump(0)
        self.assertRegex(empty_dumped_str, r"name:.*\nvariable name:.*\ncontent:.*\nfailed cases:.*\n")

        node.name = ["test_name"]
        node.var_name = ["test_var_name"]
        node.content = ["test_content"]
        node.failed_cases.append("test_failed_case")
        dump_str = node.dump(2)
        expected_str = "  name: ['test_name']\n  variable name: ['test_var_name']\n  content: ['test_content']\n  failed cases: deque(['test_failed_case'])\n"
        self.assertEqual(dump_str, expected_str)

    def test_dump_with_recurse(self):
        parent_node = parser.Node()
        child_node = parser.Node()
        child_node.name = ["child_name"]
        parent_node.children.append(child_node)
        dump_str = parent_node.dump(0, recurse=True)
        expected_str = "name: []\nvariable name: []\ncontent: []\nfailed cases: deque([])\n   name: ['child_name']\n   variable name: []\n   content: []\n   failed cases: deque([])\n"
        self.assertEqual(dump_str, expected_str)


class StrReaderTest(unittest.TestCase):

    def test_initialization(self):
        s = "line1\nline2\n  line3\n"
        reader = parser.StrReader(s)
        self.assertEqual(reader.filename, "<string>")
        self.assertEqual(len(reader._lines), 3)
        self.assertEqual(reader._lines[0], ("line1", 0, 1))
        self.assertEqual(reader._lines[1], ("line2", 0, 2))
        self.assertEqual(reader._lines[2], ("line3", 2, 3))

    def test_initialization_comments(self):
        s = "line1\nline2\n#line3\n  line4\n//line5\nline6\n"
        reader = parser.StrReader(s)
        self.assertEqual(reader.filename, "<string>")
        self.assertEqual(len(reader._lines), 4)
        self.assertEqual(reader._lines[0], ("line1", 0, 1))
        self.assertEqual(reader._lines[1], ("line2", 0, 2))
        self.assertEqual(reader._lines[2], ("line4", 2, 4))
        self.assertEqual(reader._lines[3], ("line6", 0, 6))

    def test_initialization_tabs(self):
        s = "line1\nline2  \n  line3	\n"
        reader = parser.StrReader(s)
        self.assertEqual(reader.filename, "<string>")
        self.assertEqual(len(reader._lines), 3)
        self.assertEqual(reader._lines[0], ("line1", 0, 1))
        self.assertEqual(reader._lines[1], ("line2", 0, 2))
        self.assertEqual(reader._lines[2], ("line3", 2, 3))

    def test_get_next_line(self):
        s = "line1\nline2\n  line3\n"
        reader = parser.StrReader(s)
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
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, None)
        self.assertEqual(indent, -1)
        self.assertEqual(linenum, -1)

    def test_set_next_line(self):
        s = "line1\nline2\n  line3\n"
        reader = parser.StrReader(s)
        reader.set_next_line("new line", 1, 4)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "new line")
        self.assertEqual(indent, 1)
        self.assertEqual(linenum, 4)
        line, indent, linenum = reader.get_next_line(-1)
        self.assertEqual(line, "line1")
        self.assertEqual(indent, 0)
        self.assertEqual(linenum, 1)


class FileReaderTest(unittest.TestCase):

    def test_initialization(self):
        with tempfile.NamedTemporaryFile() as temp_file:
            temp_file.write(b"line1\nline2\n  line3\n")
            temp_file.flush()
            temp_file_name = temp_file.name
            reader = parser.FileReader(temp_file_name)
        self.assertEqual(reader.filename, temp_file_name)
        self.assertEqual(len(reader._lines), 3)
        self.assertEqual(reader._lines[0], ("line1", 0, 1))
        self.assertEqual(reader._lines[1], ("line2", 0, 2))
        self.assertEqual(reader._lines[2], ("line3", 2, 3))


class ParserTest(unittest.TestCase):

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

    def test_simple_variant(self):
        self._compare_string_config("""
            c = abc
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

    def test_name_variant(self):
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

    def test_defaults(self):
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

            variants virt_system [ default=  linux ]:
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
        self.assertRaises(parser.MissingIncludeError,
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

        self.assertRaises(parser.ParserError,
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

        f = "only xxx.yyy..(xxx=333).aaa, ddd (eeee) rrr.aaa"

        self._compare_string_config(f, [], True)

        lexer = parser.Lexer(parser.StrReader(f))
        lexer.set_prev_indent(-1)
        lexer.get_next_check([parser.LIndent])
        lexer.get_next_check([parser.LOnly])
        p_filter = parser.parse_filter(lexer, lexer.rest_line())
        self.assertEqual(p_filter,
                         [[[parser.Label("xxx"),
                            parser.Label("yyy")],
                           [parser.Label("xxx", "333"),
                            parser.Label("aaa")]],
                          [[parser.Label("ddd")]],
                          [[parser.Label("eeee")]],
                          [[parser.Label("rrr"),
                            parser.Label("aaa")]]],
                         "Failed to parse filter.")

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


if __name__ == '__main__':
    unittest.main()
