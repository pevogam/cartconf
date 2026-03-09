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
parse_string = parser.parse_string
parse_file = parser.parse_file


class Parser(object):

    @property
    def node(self):
        return self.ast.clone_node(self.ast.root)

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
        self.debug = debug
        self.defaults = defaults
        self.expand_defaults = expand_defaults or []

        self.filename = filename
        if self.filename:
            self.parse_file(self.filename)

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
        self.ast = parse_file(
            self.ast,
            cfgfile,
            defaults=self.defaults,
            expand_defaults=self.expand_defaults,
        )
        self.filename = cfgfile

    def parse_string(self, cfgstr: str) -> None:
        """
        Parse a string.

        :param cfgstr: configuration string to parse
        """
        self.ast = parse_string(
            self.ast,
            cfgstr,
            defaults=self.defaults,
            expand_defaults=self.expand_defaults,
        )

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

        :returns: (recursive) dictionary generator
        """
        pre_dict = PreDict(defaults=self.defaults)
        if not pre_dict.update_from_node(self.ast.clone_node(self.ast.root)):
            # the python-rust barrier requires copying or working on copies so replace entirely
            self.ast.swap_node(pre_dict.branch[-1])
            return
        while True:
            # Since get_dicts() is recursive generator, it can invoke itself
            # and it can also be called outside to get dict generator.
            # Use special dropsufs argument to mark the top-level generator,
            # to be able to process all variables, do suffix stuff, drop dupes, etc.
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

        :returns: (recursive) dictionary generator
        """
        LOG.warning(
            "Using get_dicts() is deprecated, use get_dicts_gen() instead",
        )
        for d in self.get_dicts_gen(skipdups=skipdups):
            yield d
