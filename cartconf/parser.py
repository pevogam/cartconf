"""
Module for readers, lexers, and parsers as well as their components.
"""

import logging
import os
import threading
from typing import Generator

from .exceptions import *
from .filters import *
from .tokens import *
from .cartconf import lexer
from .cartconf import parser

LOG = logging.getLogger("avocado." + __name__)

LexerError = lexer.LexerError
Reader = lexer.Reader
Lexer = lexer.Lexer

ParserError = parser.ParserError
Label = parser.Label
Node = parser.Node
Tree = parser.Tree
PreDict = parser.PreDict


class Parser(object):

    # Cache parsing prefixes as {key: {"ast": Tree, "children": {...}}}.
    # Rust Tree objects cannot be shared across threads.
    _parse_cache = threading.local()

    @property
    def node(self):
        """Return a detached root for inspecting the parsed snapshot."""
        return self.ast.node

    @property
    def steps(self) -> tuple[tuple[str, str], ...]:
        """Return successful top-level parse calls as (kind, input) pairs."""
        return self._steps

    def __init__(
        self,
        filename: str = None,
        defaults: bool = False,
        expand_defaults: list[str] = None,
        debug: bool = False,
        cache: bool = True,
    ) -> None:
        """
        Initialize the parser.

        :param filename: file path to parse from
        :param defaults: whether to use default variants
        :param expand_defaults: list of default variants to expand
        :param debug: whether to enable debug logging
        :param cache: whether to reuse and store ASTs for matching parsing prefixes
        """
        self.ast = Tree()
        self._initial_ast = self.ast
        self._cache_keys = ()
        self._steps = ()
        self.debug = debug
        self.defaults = defaults
        self.expand_defaults = list(expand_defaults or [])
        self.cache = cache

        self.filename = filename
        if self.filename:
            self.parse_file(self.filename)

    def __copy__(self) -> "Parser":
        """Share immutable syntax and history, with independent parser options."""
        new = object.__new__(type(self))
        new.__dict__ = self.__dict__.copy()
        new.expand_defaults = self.expand_defaults.copy()
        return new

    @classmethod
    def clear_cache(cls) -> None:
        """Discard this thread's cached ASTs, for example after editing configs."""
        cls._parse_cache.entries = {}

    def _get_cache(self) -> dict | None:
        """Find cached children only when the prefix still identifies this AST."""
        if not hasattr(self._parse_cache, "entries"):
            self.clear_cache()
        cache_ref = self._parse_cache.entries
        cached_ast = self._initial_ast
        for key in self._cache_keys:
            entry = cache_ref.get(key)
            if entry is None:
                return None
            cached_ast = entry["ast"]
            cache_ref = entry["children"]
        # Directly assigning ast must not reuse or populate another prefix.
        return cache_ref if cached_ast is self.ast else None

    def _debug(self, s, *args):
        if self.debug:
            LOG.debug(s, *args)

    def _warn(self, s, *args):
        LOG.warn(s, *args)

    def parse_file(self, cfgfile: str) -> None:
        """
        Parse a file.

        :param cfgfile: configuration file path to parse
        """
        self._parse("file", cfgfile)

    def parse_string(self, cfgstr: str) -> None:
        """
        Parse a string.

        :param cfgstr: configuration string to parse
        """
        self._parse("string", cfgstr)

    def parse_dict(self, cfgdict: dict[str, str]) -> None:
        """
        Parse dictionary entries as ordered configuration assignments.

        :param cfgdict: parameters to add or overwrite

        Values use the same quoting and substitution rules as parse_string().
        The history retains assignment text independently of the input dictionary.
        """
        self._parse("dict", "".join("%s = %s\n" % item for item in cfgdict.items()))

    def _parse(self, kind: str, content: str) -> None:
        """Extend the snapshot and record an input only after successful parsing."""
        key = (
            kind,
            content,
            self.defaults,
            tuple(self.expand_defaults),
            os.getcwd(),
        )
        cache_ref = self._get_cache() if self.cache else None
        entry = cache_ref.get(key) if cache_ref is not None else None
        if entry is None:
            parse = self.ast.parse_file if kind == "file" else self.ast.parse_string
            ast = parse(
                content,
                defaults=self.defaults,
                expand_defaults=self.expand_defaults,
            )
            if cache_ref is not None:
                cache_ref[key] = {"ast": ast, "children": {}}
        else:
            ast = entry["ast"]
        self.ast = ast
        if kind == "file":
            self.filename = content
        self._steps += ((kind, content),)
        self._cache_keys += (key,)

    def only_filter(self, variant: str) -> None:
        """
        Apply a only filter programmatically and keep track of it.

        Equivalent to parse a "only variant" line.

        :param variant: variant name to filter with
        """
        string = "only %s" % variant
        self.parse_string(string)

    def no_filter(self, variant: str) -> None:
        """
        Apply a no filter programmatically and keep track of it.

        Equivalent to parse a "no variant" line.

        :param variant: variant name to filter with
        """
        string = "no %s" % variant
        self.parse_string(string)

    def assign(self, key: str, value: str) -> None:
        """
        Apply an assignment programmatically and keep track of it.

        Equivalent to parse a "key = value" line.

        :param key: key to assign to
        :param value: value to assign
        """
        string = "%s = %s" % (key, value)
        self.parse_string(string)

    def get_dicts_gen(
        self,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get dictionaries from a parser and given or its current node.

        :returns: dictionary generator
        """
        if self.ast.is_empty():
            return
        pre_dict = PreDict(defaults=self.defaults)
        if not pre_dict.update_from_tree(self.ast):
            return
        while True:
            d = pre_dict.get_dicts(dropsufs=True, skipdups=skipdups)
            if d is None:
                break
            yield d

    def get_dicts(
        self,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get dictionaries from a parser and given or its current node (legacy).

        :returns: dictionary generator
        """
        LOG.warning(
            "Using get_dicts() is deprecated, use get_dicts_gen() instead",
        )
        for d in self.get_dicts_gen(skipdups=skipdups):
            yield d
