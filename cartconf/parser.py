"""
Module for readers, lexers, and parsers as well as their components.
"""

import os
import logging
import re
from typing import Generator

from .exceptions import *
from .utils import drop_suffixes, apply_suffix_bounds
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
parse_string = parser.parse_string
parse_file = parser.parse_file


class PreDict(object):

    def __init__(self, ctx=None, content=None, shortname=None, dep=None):
        self.ctx: list[Label] = ctx or []
        self.content: list[tuple[str, int, "Token"]] = content or []
        self.shortname: list[Label] = shortname or []
        self.dep: list[str] = dep or []

    def get_dict(self):
        d = {
            "name": ".".join([str(label) for label in self.ctx]),
            "dep": self.dep,
            "shortname": ".".join([str(sn.name) for sn in self.shortname]),
        }
        for _, _, op in self.content:
            op.apply_to_dict(d)
        return d


class Parser(object):
    # pylint: disable=W0102

    num_failed_cases = 5

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
        self.node = Node()
        self.debug = debug
        self.defaults = defaults
        self.expand_defaults = expand_defaults or []

        self.filename = filename
        if self.filename:
            self.parse_file(self.filename)

        # get_dicts_joined() - is recursive generator, it can invoke itself,
        # as well as it can be called outside to get dict list
        # It is necessary somehow to mark the top-level generator,
        # to be able to process all variables, do suffix stuff, drops dupes, etc....
        # It can be safely done only on the top level get_dicts_joined()
        # Parent generator will reset this flag
        self.parent_generator = True

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
        self.node.filename = cfgfile
        self.node = parse_file(
            cfgfile,
            self.node,
            defaults=self.defaults,
            expand_defaults=self.expand_defaults,
        )
        self.filename = cfgfile

    def parse_string(self, cfgstr: str) -> None:
        """
        Parse a string.

        :param cfgstr: configuration string to parse
        """
        self.node.filename = Reader(content="").filename
        self.node = parse_string(
            cfgstr,
            self.node,
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

    def get_dicts(
        self,
        pre_dict: PreDict = None,
        node: Node = None,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get dictionaries from a parser and given or its current node.

        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: (recursive) dictionary generator
        """
        return self.get_dicts_joined(pre_dict, node, skipdups)

    def get_dicts_plain(
        self,
        pre_dict: PreDict = None,
        node: Node = None,
    ) -> Generator[dict[str, str], None, None]:
        """
        Generate dictionaries from the code parsed so far.

        This should be called after parsing something.

        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: (recursive) dictionary generator
        """
        pre_dict = pre_dict or PreDict()
        ctx, shortname, dep = pre_dict.ctx, pre_dict.shortname, pre_dict.dep
        content = pre_dict.content
        node = node or self.node

        # if self.debug:    #Print dict on which is working now.
        #    print(node.dump(0))
        # Update dep
        for d in node.dep:
            for dd in d:
                dep = dep + [".".join([str(label) for label in ctx + dd])]
        # Update ctx
        ctx = ctx + node.name
        labels = list(node.labels)
        # Get the current name
        name = ".".join([str(label) for label in ctx])

        if node.name:
            self._debug("checking out %r", name)

        # Check previously failed filters
        for i, failed_case in enumerate(node.get_failed_cases()):
            if not node.failed_case_might_pass(i, ctx, labels, content):
                self._debug(
                    "\n*    this subtree has failed before %s\n"
                    "         content: %s\n"
                    "         failcase:%s\n",
                    name,
                    content + node.get_content(),
                    failed_case,
                )
                node.update_failed_case(i, *failed_case)
                return

        # Check content and unpack it into new_content
        internal_content, failed_internal_filters, failed_cond_filters = (
            node.process_content(ctx, labels)
        )
        failed_internal_filters += failed_cond_filters
        content_node = Node()
        content_node.swap_content(content)
        external_content, failed_external_filters, failed_cond_filters = (
            content_node.process_content(ctx, labels)
        )
        # NOTE: the failed filters should go into the failed internal filters
        # because we don't expect them to come from outside this node, even if
        # the condition itself was external
        failed_internal_filters += failed_cond_filters
        new_content = internal_content + external_content
        if failed_internal_filters or failed_external_filters:
            node.add_failed_case(
                ctx,
                failed_external_filters,
                failed_external_filters,
                Parser.num_failed_cases,
            )
            self._debug("Failed_cases %s", node.get_failed_cases())
            return

        # Update shortname
        if node.append_to_shortname:
            shortname = shortname + node.name

        # Recurse into children
        count = 0
        new_pre_dict = PreDict(ctx, new_content, shortname, dep)
        if self.defaults and ".".join(str(node.var_name)) not in self.expand_defaults:
            for n in node.get_children():
                for d in self.get_dicts_joined(new_pre_dict, n):
                    count += 1
                    yield d
                if n.default and count:
                    break
        else:
            for n in node.get_children():
                for d in self.get_dicts_joined(new_pre_dict, n):
                    count += 1
                    yield d
        # Reached leaf?
        if not node.get_children():
            self._debug("    reached leaf, returning it")
            d = new_pre_dict.get_dict()
            apply_suffix_bounds(d)
            yield d

    def get_dicts_joined(
        self,
        pre_dict: PreDict = None,
        node: Node = None,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get possibly joined dictionaries added using only filters.

        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: (recursive) dictionary generator

        Process 'join' entries and unpack join filters in the node.

        Main rules for joining via filters:

        1) join filter_1 filter_2 ....
            multiplies all dictionaries as:
                all_variants_match_filter_1 * all_variants_match_filter_2 * ....
        2) join only_one_filter
                == only only_one_filter
        3) join filter_1 filter_1
            also works and transforms to:
                all_variants_match_filter_1 * all_variants_match_filter_1
            Example:
                join a
                join a
            Transforms into:
                join a a
        """
        pre_dict = pre_dict or PreDict()
        node = node or self.node

        # Keep track to know who is a parent generator
        parent = False
        if self.parent_generator:
            # I am parent of the all
            parent = True
            # No one else is
            self.parent_generator = False

        # Node is a current block. It has content, its contents: node.get_content()
        # Content without joins
        new_content = []

        # All joins in current node
        joins = []

        for t in node.get_content():
            filename, linenum, obj = t

            if not isinstance(obj, JoinFilter):
                new_content.append(t)
                continue

            # Accumulate all joins at one node
            joins += [t]

        if not joins:
            # Return generator
            for d in self.get_dicts_plain(pre_dict, node):
                yield drop_suffixes(d, skipdups=skipdups) if parent else d
        else:
            # Rewrite all separate joins in one node as many `only'
            onlys = []
            for j in joins:
                filename, linenum, obj = j
                for word in obj.filter:
                    f = OnlyFilter([word], str(word))
                    onlys += [(filename, linenum, f)]

            old_content = node.get_content()
            node.swap_content(new_content)
            for d in self.join_filters(onlys, pre_dict, node):
                yield drop_suffixes(d, skipdups=skipdups) if parent else d
            node.swap_content(old_content[:])

    def join_filters(
        self,
        onlys: list[tuple[str, int, Filter]],
        pre_dict: PreDict = None,
        node: Node = None,
    ) -> Generator[dict[str, str], None, None]:
        """
        Perform all joins as filters on added dictionaries.

        :param onlys: list of only filters
        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: (recursive) dictionary generator

        Each `join' is the same as an `only' filter.
        """
        pre_dict = pre_dict or PreDict()
        node = node or self.node

        # Current join/only
        only = onlys[:1]
        remains = onlys[1:]

        content_orig = node.get_content()
        for f, i, obj in only:
            node.add_content(f, i, obj)

        if not remains:
            for d in self.get_dicts_plain(pre_dict, node):
                yield d
        else:
            for d1 in self.get_dicts_plain(pre_dict, node):
                # Current frame multiply by all variants from bottom
                node.swap_content(content_orig)
                for d2 in self.join_filters(remains, pre_dict, node):

                    d = d1.copy()
                    d.update(d2)
                    d["name"] = Node.join_names(d1["name"], d2["name"])
                    d["shortname"] = Node.join_names(d1["shortname"], d2["shortname"])
                    yield d
