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
        node: Node = None,
        ctx: list[Label] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[Label] = None,
        dep: list[str] = None,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get dictionaries from a parser and given or its current node.

        :param node: node to start from
        :param ctx: node labels/names
        :param content: previous content in plain
        :param shortname: short name
        :param dep: dependencies
        :returns: dictionary generator
        """
        node = node or self.node
        ctx = ctx or []
        content = content or []
        shortname = shortname or []
        dep = dep or []
        return self.get_dicts_joined(node, ctx, content, shortname, dep, skipdups)

    def get_dicts_plain(
        self,
        node: Node = None,
        ctx: list[Label] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[Label] = None,
        dep: list[str] = None,
    ) -> Generator[dict[str, str], None, None]:
        """
        Generate dictionaries from the code parsed so far.

        This should be called after parsing something.

        :param node: node to start from
        :param ctx: node labels/names
        :param content: previous content in plain
        :param shortname: short name
        :param dep: dependencies
        :returns: dictionary generator
        """
        node = node or self.node
        ctx = ctx or []
        content = content or []
        shortname = shortname or []
        dep = dep or []

        def process_content(content, failed_filters):
            # 1. Check that the filters in content are OK with the current
            #    context (ctx).
            # 2. Move the parts of content that are still relevant into
            #    new_content and unpack conditional blocks if appropriate.
            #    For example, if an 'only' statement fully matches ctx, it
            #    becomes irrelevant and is not appended to new_content.
            #    If a conditional block fully matches, its contents are
            #    unpacked into new_content.
            # 3. Move failed filters into failed_filters, so that next time we
            #    reach this node or one of its ancestors, we'll check those
            #    filters first.
            blocked_filters = []
            for t in content:
                filename, linenum, obj = t
                if tokens_oper_key(obj) in list(tokens_oper):
                    new_content.append(t)
                    continue
                filter = (
                    obj.condition
                    if hasattr(obj, "condition") and obj.condition is not None
                    else obj
                )
                # obj is an OnlyFilter/NoFilter/Condition/NegativeCondition
                if filter.requires_action(ctx, labels):
                    # This filter requires action now
                    if type(filter) is OnlyFilter or type(filter) is NoFilter:
                        if filter not in blocked_filters:
                            self._debug(
                                "    filter did not pass: %r (%s:%s)",
                                filter.line,
                                filename,
                                linenum,
                            )
                            failed_filters += [t]
                            return False
                        else:
                            continue
                    else:
                        self._debug(
                            "    conditional block matches:" " %r (%s:%s)",
                            filter.line,
                            filename,
                            linenum,
                        )
                        # Check and unpack the content inside this Condition
                        # object (note: the failed filters should go into
                        # new_internal_filters because we don't expect them to
                        # come from outside this node, even if the Condition
                        # itself was external)
                        if not process_content(obj.get_content(), new_internal_filters):
                            failed_filters += [t]
                            return False
                        continue
                elif filter.is_irrelevant(ctx, labels):
                    # This filter is no longer relevant and can be removed
                    continue
                else:
                    # Keep the filter and check it again later
                    new_content.append(t)
            return True

        def might_pass(failed_ctx, failed_external_filters, failed_internal_filters):
            all_content = content + node.get_content()
            for t in failed_external_filters + failed_internal_filters:
                if t not in all_content:
                    return True
            for t in failed_external_filters:
                _, _, external_filter = t
                if not external_filter.might_pass(failed_ctx, ctx, labels):
                    return False
            for t in failed_internal_filters:
                if t not in node.get_content():
                    return True

            for t in failed_internal_filters:
                _, _, internal_filter = t
                if not internal_filter.might_pass(failed_ctx, ctx, labels):
                    return False
            return True

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
            if not might_pass(*failed_case):
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
        new_content = []
        new_external_filters = []
        new_internal_filters = []
        if not process_content(
            node.get_content(), new_internal_filters
        ) or not process_content(content, new_external_filters):
            node.add_failed_case(
                ctx,
                new_external_filters,
                new_internal_filters,
                Parser.num_failed_cases,
            )
            self._debug("Failed_cases %s", node.get_failed_cases())
            return

        # Update shortname
        if node.append_to_shortname:
            shortname = shortname + node.name

        # Recurse into children
        count = 0
        if self.defaults and ".".join(str(node.var_name)) not in self.expand_defaults:
            for n in node.get_children():
                for d in self.get_dicts_joined(n, ctx, new_content, shortname, dep):
                    count += 1
                    yield d
                if n.default and count:
                    break
        else:
            for n in node.get_children():
                for d in self.get_dicts_joined(n, ctx, new_content, shortname, dep):
                    count += 1
                    yield d
        # Reached leaf?
        if not node.get_children():
            self._debug("    reached leaf, returning it")
            d = {
                "name": name,
                "dep": dep,
                "shortname": ".".join([str(sn.name) for sn in shortname]),
            }
            for _, _, op in new_content:
                op.apply_to_dict(d)
            apply_suffix_bounds(d)
            yield d

    def get_dicts_joined(
        self,
        node: Node = None,
        ctx: list[Label] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[Label] = None,
        dep: list[str] = None,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get possibly joined dictionaries added using only filters.

        :param node: node to start from
        :param ctx: node labels/names
        :param content: previous content in plain
        :param shortname: short name
        :param dep: dependencies
        :returns: dictionary generator

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
        node = node or self.node
        ctx = ctx or []
        content = content or []
        shortname = shortname or []
        dep = dep or []

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
            for d in self.get_dicts_plain(node, ctx, content, shortname, dep):
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
            for d in self.join_filters(onlys, node, ctx, content, shortname, dep):
                yield drop_suffixes(d, skipdups=skipdups) if parent else d
            node.swap_content(old_content[:])

    def join_names(self, n1: str, n2: str) -> str:
        """
        Produce a new name from two old names where two dictionaries were joined.

        :param n1: name of the first dictionary
        :param n2: name of the second dictionary
        :returns: a new name reusing variant names
        """
        common_prefix = n1[: [x[0] == x[1] for x in list(zip(n1, n2))].index(0)]
        cp = ".".join(common_prefix.split(".")[:-1])
        p1 = re.sub(r"^" + cp, "", n1)
        p2 = re.sub(r"^" + cp, "", n2)
        if cp:
            name = cp + p1 + p2
        else:
            name = p1 + "." + p2
        return name

    def join_filters(
        self,
        onlys: list[tuple[str, int, Filter]],
        node: Node = None,
        ctx: list[Label] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[Label] = None,
        dep: list[str] = None,
    ) -> Generator[dict[str, str], None, None]:
        """
        Perform all joins as filters on added dictionaries.

        :param onlys: list of only filters
        :param node: node to start from
        :param ctx: node labels/names
        :param content: previous content in plain
        :param shortname: short name
        :param dep: dependencies
        :returns: (resursive) dictionary generator

        Each `join' is the same as an `only' filter.
        """
        node = node or self.node
        ctx = ctx or []
        content = content or []
        shortname = shortname or []
        dep = dep or []

        # Current join/only
        only = onlys[:1]
        remains = onlys[1:]

        content_orig = node.get_content()
        for f, i, obj in only:
            node.add_content(f, i, obj)

        if not remains:
            for d in self.get_dicts_plain(node, ctx, content, shortname, dep):
                yield d
        else:
            for d1 in self.get_dicts_plain(node, ctx, content, shortname, dep):
                # Current frame multiply by all variants from bottom
                node.swap_content(content_orig)
                for d2 in self.join_filters(
                    remains, node, ctx, content, shortname, dep
                ):

                    d = d1.copy()
                    d.update(d2)
                    d["name"] = self.join_names(d1["name"], d2["name"])
                    d["shortname"] = self.join_names(d1["shortname"], d2["shortname"])
                    yield d
