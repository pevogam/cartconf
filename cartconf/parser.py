"""
Module for readers, lexers, and parsers as well as their components.
"""

import logging
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
    ) -> None:
        """
        Initialize the parser.

        :param filename: file path to parse from
        :param defaults: whether to use default variants
        :param expand_defaults: list of default variants to expand
        :param debug: whether to enable debug logging
        """
        self.ast = Tree()
        self._steps = ()
        self.debug = debug
        self.defaults = defaults
        self.expand_defaults = list(expand_defaults or [])

        self.only_filters = []
        self.no_filters = []
        self.assignments = []

        self.filename = filename
        if self.filename:
            self.parse_file(self.filename)

    def __copy__(self) -> "Parser":
        """Share immutable syntax and history, with independent parser options."""
        new = object.__new__(type(self))
        new.__dict__ = self.__dict__.copy()
        new.expand_defaults = self.expand_defaults.copy()
        return new

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
        self.ast = self.ast.parse_file(
            cfgfile,
            defaults=self.defaults,
            expand_defaults=self.expand_defaults,
        )
        self.filename = cfgfile
        self._steps += (("file", cfgfile),)

    def parse_string(self, cfgstr: str) -> None:
        """
        Parse a string.

        :param cfgstr: configuration string to parse
        """
        self.ast = self.ast.parse_string(
            cfgstr,
            defaults=self.defaults,
            expand_defaults=self.expand_defaults,
        )
        self._steps += (("string", cfgstr),)

    def only_filter(self, variant: str) -> None:
        """
        Apply a only filter programmatically and keep track of it.

        Equivalent to parse a "only variant" line.

        :param variant: variant name to filter with
        """
        string = "only %s" % variant
        self.only_filters.append(string)
        self.parse_string(string)

    def no_filter(self, variant: str) -> None:
        """
        Apply a no filter programmatically and keep track of it.

        Equivalent to parse a "no variant" line.

        :param variant: variant name to filter with
        """
        string = "no %s" % variant
        self.no_filters.append(string)
        self.parse_string(string)

    def assign(self, key: str, value: str) -> None:
        """
        Apply an assignment programmatically and keep track of it.

        Equivalent to parse a "key = value" line.

        :param key: key to assign to
        :param value: value to assign
        """
        string = "%s = %s" % (key, value)
        self.assignments.append(string)
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
            # Flatten suffixes after the traversal has combined any join components.
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
