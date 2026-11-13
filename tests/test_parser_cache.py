import copy
import os
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from cartconf.parser import Parser, ParserError


class ParserCacheTest(unittest.TestCase):
    def setUp(self):
        Parser.clear_cache()
        self.addCleanup(Parser.clear_cache)

    def test_cached_prefixes_and_branches(self):
        """Reuse files, strings and dictionaries while keeping siblings separate."""
        with tempfile.TemporaryDirectory() as directory:
            filename = os.path.join(directory, "base.cfg")
            Path(directory, "included.cfg").write_text(
                "variants:\n    - a:\n    - b:\n"
            )
            Path(filename).write_text("include included.cfg\n")
            left = Parser(filename)
        file_ast = left.ast
        left.parse_string("only a\n")
        string_ast = left.ast
        left.parse_dict({"left": "1"})
        expected = list(left.get_dicts_gen())

        # Both files are gone: success requires reuse of the cached file prefix.
        repeated = Parser(filename, debug=True)
        self.assertIs(repeated.ast, file_ast)
        self.assertEqual(repeated.filename, filename)
        repeated.parse_string("only a\n")
        self.assertIs(repeated.ast, string_ast)
        right = copy.copy(repeated)
        repeated.parse_dict({"left": "1"})
        self.assertIs(repeated.ast, left.ast)
        self.assertEqual(repeated.steps, left.steps)
        self.assertEqual(list(repeated.get_dicts_gen()), expected)
        self.assertTrue(repeated.debug)
        self.assertFalse(left.debug)

        right.parse_dict({"right": "2"})
        self.assertNotIn("left", next(right.get_dicts_gen()))
        self.assertNotIn("right", next(left.get_dicts_gen()))
        cached_right = Parser(filename)
        cached_right.parse_string("only a\n")
        cached_right.parse_dict({"right": "2"})
        self.assertIs(cached_right.ast, right.ast)

        left.parse_dict({"later": "3"})
        self.assertEqual(list(repeated.get_dicts_gen()), expected)

    def test_cached_traversals_are_independent(self):
        """Cached syntax shares neither iterator progress nor failed filters."""
        config = "variants:\n    - a:\n    - b:\n"
        first, second = Parser(), Parser()
        first.parse_string(config)
        second.parse_string(config)
        self.assertIs(first.ast, second.ast)
        expected = list(first.get_dicts_gen())
        active = first.get_dicts_gen()
        self.assertEqual(next(active), expected[0])
        second.parse_string("only missing\n")
        self.assertEqual(list(second.get_dicts_gen()), [])
        self.assertEqual(list(active), expected[1:])
        self.assertEqual(list(first.get_dicts_gen()), expected)
        restored = Parser()
        restored.parse_string(config)
        self.assertIs(restored.ast, first.ast)
        self.assertEqual(list(restored.get_dicts_gen()), expected)

    def test_cache_options(self):
        """Defaults and expanded groups select distinct, reusable ASTs."""
        config = (
            "variants guest_os [default=linux]:\n"
            "    - linux:\n    - windows:\n"
            "variants host_os:\n    - linux:\n    - @windows:\n"
        )
        systems = ["linux", "windows"]
        cases = (
            (False, [], systems, systems),
            (True, [], ["windows"], ["linux"]),
            (True, ["guest_os"], ["windows"], systems),
            (True, ["host_os"], systems, ["linux"]),
            (True, ["host_os", "guest_os"], systems, systems),
        )
        with tempfile.NamedTemporaryFile(mode="w") as config_file:
            config_file.write(config)
            config_file.flush()
            for from_file in (False, True):
                for defaults, expanded, hosts, guests in cases:
                    with self.subTest(
                        from_file=from_file, defaults=defaults, expanded=expanded
                    ):
                        snapshots = []
                        for _ in range(2):
                            parsed = Parser(defaults=defaults, expand_defaults=expanded)
                            if from_file:
                                parsed.parse_file(config_file.name)
                            else:
                                parsed.parse_string(config)
                            self.assertEqual(
                                [
                                    (d["host_os"], d["guest_os"])
                                    for d in parsed.get_dicts_gen()
                                ],
                                [(host, guest) for host in hosts for guest in guests],
                            )
                            snapshots.append(parsed.ast)
                        self.assertIs(snapshots[0], snapshots[1])

    def test_options_are_recorded_for_each_step(self):
        """Equal final options must not erase differences in prefix options."""
        config = "variants guest_os [default=linux]:\n    - linux:\n    - windows:\n"
        snapshots = []
        cases = ((False, [], 1), (True, ["guest_os"], 2), (True, [], 1))
        for defaults, expanded, count in cases:
            with self.subTest(defaults=defaults, expanded=expanded):
                for repeat in range(2):
                    parsed = Parser(defaults=defaults, expand_defaults=expanded)
                    parsed.parse_string(config)
                    parsed.defaults = True
                    parsed.expand_defaults.clear()
                    parsed.parse_dict({"value": "final"})
                    self.assertEqual(len(list(parsed.get_dicts_gen())), count)
                    if repeat:
                        self.assertIs(parsed.ast, snapshots[-1])
                    else:
                        for previous in snapshots:
                            self.assertIsNot(parsed.ast, previous)
                        snapshots.append(parsed.ast)

    def test_input_order(self):
        """Identical suffixes following different prefixes use distinct ASTs."""
        for values in (("one", "two"), ("two", "one")):
            with self.subTest(values=values):
                snapshots = []
                for _ in range(2):
                    parsed = Parser()
                    for value in values:
                        parsed.parse_dict({"value": value})
                    parsed.parse_string("reference = ${value}\n")
                    self.assertEqual(
                        next(parsed.get_dicts_gen())["reference"], values[-1]
                    )
                    snapshots.append(parsed.ast)
                self.assertIs(snapshots[0], snapshots[1])

    def test_input_kinds(self):
        """File, string and dictionary calls keep their own source history."""
        with tempfile.NamedTemporaryFile(mode="w") as config:
            config.write("value = original\n")
            config.flush()
            calls = (
                ("file", config.name),
                ("string", "include %s\n" % config.name),
                ("string", "value = original\n"),
                ("dict", {"value": "original"}),
            )
            for kind, content in calls:
                with self.subTest(kind=kind, content=content):
                    first, second = Parser(), Parser()
                    for parsed in (first, second):
                        getattr(parsed, "parse_" + kind)(content)
                        self.assertEqual(parsed.steps[0][0], kind)
                        self.assertEqual(
                            next(parsed.get_dicts_gen())["value"], "original"
                        )
                        self.assertEqual(
                            parsed.filename, config.name if kind == "file" else None
                        )
                    self.assertIs(first.ast, second.ast)

    def test_failed_parse_does_not_poison_cache(self):
        """Failed extensions preserve the prefix and allow a corrected retry."""
        parsed = Parser()
        parsed.parse_dict({"value": "original"})
        snapshot, steps = parsed.ast, parsed.steps
        with tempfile.NamedTemporaryFile(mode="w") as config:
            config.write("value = changed\nbroken ?! value\n")
            config.flush()
            for kind, content in (
                ("file", config.name),
                ("string", "value = changed\nbroken ?! value\n"),
                ("dict", {"value": "changed\nbroken ?! value"}),
            ):
                with self.subTest(kind=kind):
                    with self.assertRaises(ParserError):
                        getattr(parsed, "parse_" + kind)(content)
                    self.assertIs(parsed.ast, snapshot)
                    self.assertIs(parsed.steps, steps)
                    self.assertIsNone(parsed.filename)
            config.seek(0)
            config.truncate()
            config.write("value = corrected\n")
            config.flush()
            parsed.parse_file(config.name)
        repeated = Parser()
        repeated.parse_dict({"value": "original"})
        repeated.parse_file(config.name)
        self.assertIs(repeated.ast, parsed.ast)
        self.assertEqual(next(repeated.get_dicts_gen())["value"], "corrected")

    def test_bypass_and_clear_cache(self):
        """Explicit bypass and clearing refresh files and their includes."""
        with tempfile.TemporaryDirectory() as directory:
            filename = os.path.join(directory, "base.cfg")
            included = Path(directory, "included.cfg")
            Path(filename).write_text("include included.cfg\n")
            included.write_text("value = original\n")
            cached = Parser(filename)
            included.write_text("value = updated\n")
            fresh = Parser(filename, cache=False)
            self.assertEqual(next(fresh.get_dicts_gen())["value"], "updated")
            self.assertIsNot(fresh.ast, cached.ast)
            self.assertIs(Parser(filename).ast, cached.ast)
            Parser.clear_cache()
            refreshed = Parser(filename)
            self.assertEqual(next(refreshed.get_dicts_gen())["value"], "updated")
            self.assertIsNot(refreshed.ast, cached.ast)
            self.assertEqual(next(cached.get_dicts_gen())["value"], "original")

            # The old parser's prefix no longer matches the rebuilt cache.
            refreshed.parse_dict({"extra": "1"})
            cached.parse_dict({"extra": "1"})
            self.assertEqual(next(cached.get_dicts_gen())["value"], "original")
            self.assertEqual(next(refreshed.get_dicts_gen())["value"], "updated")

    def test_direct_ast_assignment(self):
        """Manually replaced snapshots neither read nor pollute prefix entries."""
        source = Parser(cache=False)
        source.parse_dict({"external": "retained"})
        for has_prefix in (False, True):
            with self.subTest(has_prefix=has_prefix):
                normal = Parser()
                if has_prefix:
                    normal.parse_string("value = prefix\n")
                replaced = copy.copy(normal)
                normal.parse_dict({"value": "child"})
                replaced.ast = source.ast
                replaced.parse_dict({"value": "child"})
                replaced.parse_dict({"later": "1"})
                self.assertEqual(next(replaced.get_dicts_gen())["external"], "retained")
                self.assertNotIn("external", next(normal.get_dicts_gen()))
                repeated = Parser()
                if has_prefix:
                    repeated.parse_string("value = prefix\n")
                repeated.parse_dict({"value": "child"})
                self.assertIs(repeated.ast, normal.ast)
                self.assertNotIn("external", next(repeated.get_dicts_gen()))

    def test_relative_paths(self):
        """Relative files and includes resolve separately in each working directory."""
        original_directory = os.getcwd()
        try:
            with tempfile.TemporaryDirectory() as directory:
                for value in ("left", "right"):
                    location = Path(directory, value)
                    location.mkdir()
                    (location / "base.cfg").write_text("include included.cfg\n")
                    (location / "included.cfg").write_text("value = %s\n" % value)
                    os.chdir(location)
                    for kind, content in (
                        ("file", "base.cfg"),
                        ("string", "include included.cfg\n"),
                    ):
                        with self.subTest(value=value, kind=kind):
                            snapshots = []
                            for _ in range(2):
                                parsed = Parser()
                                getattr(parsed, "parse_" + kind)(content)
                                self.assertEqual(
                                    next(parsed.get_dicts_gen())["value"], value
                                )
                                snapshots.append(parsed.ast)
                            self.assertIs(snapshots[0], snapshots[1])
                    os.chdir(original_directory)
        finally:
            os.chdir(original_directory)

    def test_thread_isolation(self):
        """Independent threads cache locally because Rust ASTs are not sendable."""
        main = Parser()
        main.parse_dict({"value": "shared input"})

        def parse_in_worker():
            first, second = Parser(), Parser()
            first.parse_dict({"value": "shared input"})
            second.parse_dict({"value": "shared input"})
            return next(second.get_dicts_gen())["value"], first.ast is second.ast

        with ThreadPoolExecutor(max_workers=1) as executor:
            self.assertEqual(
                executor.submit(parse_in_worker).result(), ("shared input", True)
            )
        repeated = Parser()
        repeated.parse_dict({"value": "shared input"})
        self.assertIs(repeated.ast, main.ast)


if __name__ == "__main__":
    unittest.main()
