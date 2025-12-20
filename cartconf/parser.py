"""
Module for readers, lexers, and parsers as well as their components.
"""

import logging
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

    num_failed_cases = 5

    @property
    def ctx(self):
        return [label for ctx in self._ctx for label in ctx]

    @property
    def content(self):
        return [step for content in self._content for step in content]

    @property
    def final_content(self):
        return self._content[-1] + self._ctx_content[-1]

    @property
    def shortname(self):
        return [label for shortname in self._shortname for label in shortname]

    @property
    def dep(self):
        return [s for dep in self._dep for s in dep]

    def __init__(
        self,
        ctx: list[Label] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[Label] = None,
        dep: list[str] = None,
    ) -> None:
        self._ctx: list[list[Label]] = [ctx] if ctx else [[]]
        self._content: list[list[tuple[str, int, "Token"]]] = (
            [content] if content else [[]]
        )
        self._ctx_content: list[list[tuple[str, int, "Token"]]] = [[]]
        self._shortname: list[list[Label]] = [shortname] if shortname else [[]]
        self._dep: list[list[str]] = [dep] if dep else [[]]

        self.branch: list[Node] = []
        self.route: list[int | None] = []
        self.joins: list[Generator[dict[str, str], None, None]] = []

    def __str__(self) -> str:
        return f"PreDict(ctx={self.ctx}, content={self.content}, shortname={self.shortname}, dep={self.dep})"

    def copy(self) -> "PreDict":
        new = PreDict()

        new._ctx = self._ctx.copy()
        new._content = self._content.copy()
        new._ctx_content = self._ctx_content.copy()
        new._shortname = self._shortname.copy()
        new._dep = self._dep.copy()

        new.branch = self.branch.copy()
        new.route = self.route.copy()
        new.joins = self.joins.copy()

        return new

    def update_from_node(self, node: Node) -> bool:
        ctx, shortname, dep = [], [], []
        content = self.final_content
        # if self.debug:    #Print dict on which is working now.
        #    print(node.dump(0))
        # Update dep
        for d in node.dep:
            for dd in d:
                dep += [".".join([str(label) for label in self.ctx + dd])]
        # Update ctx
        ctx += node.name
        labels = list(node.labels)
        # Update shortname
        if node.append_to_shortname:
            shortname += node.name

        # if node.name:
        #    self._debug("checking out %r", name)

        # Check previously failed filters
        for i in range(len(node.get_failed_cases())):
            if not node.failed_case_might_pass(i, self.ctx + ctx, labels, self.content):
                # self._debug(
                #    "\n*    this subtree has failed before %s\n"
                #    "         content: %s\n"
                #    "         failcase:%s\n",
                #    name,
                #    self.content + node.get_content(),
                #    failed_case,
                # )
                node.prioritize_failed_case(i)
                return False

        self._ctx += [ctx]
        self._content += [[]]
        self._ctx_content += [[]]
        self._shortname += [shortname]
        self._dep += [dep]

        self.branch += [node]
        self.route += [None]
        self.joins += [None]

        # Check content and unpack it into new content
        internal_content, failed_internal_filters, failed_cond_filters = (
            node.process_content(self.ctx + ctx, labels)
        )
        failed_internal_filters += failed_cond_filters
        content_node = Node()
        content_node.swap_content(content)
        external_content, failed_external_filters, failed_cond_filters = (
            content_node.process_content(self.ctx + ctx, labels)
        )

        # update redundant filters from updated external content
        self._ctx_content[-1] = external_content
        self._content[-1] = internal_content

        # NOTE: the failed filters should go into the failed internal filters
        # because we don't expect them to come from outside this node, even if
        # the condition itself was external
        failed_internal_filters += failed_cond_filters
        if failed_internal_filters or failed_external_filters:
            node.add_failed_case(
                self.ctx,
                failed_external_filters,
                failed_internal_filters,
                self.num_failed_cases,
            )
            # self._debug("Failed_cases %s", node.get_failed_cases())
            return False

        return True

    def reset_from_last_node(self):
        self.branch.pop()
        self.route.pop()
        self.joins.pop()
        # should only pop these if route could be popped
        self._ctx.pop()
        self._ctx_content.pop()
        self._content.pop()
        self._shortname.pop()
        self._dep.pop()

    def get_dict(self) -> dict[str, str]:
        d = {
            "name": ".".join([str(label) for label in self.ctx]),
            "dep": self.dep,
            "shortname": ".".join([str(sn.name) for sn in self.shortname]),
        }
        for _, _, op in self.final_content:
            op.apply_to_dict(d)
        return d


class Parser(object):

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
        pre_dict = pre_dict or PreDict()
        while True:
            # mark current call as top-level parent generator for proper behavior
            self.parent_generator = True
            d = self.get_dicts_joined(pre_dict, node, skipdups=skipdups)
            if d is None:
                break
            yield drop_suffixes(d, skipdups=skipdups)

    def get_dicts_plain(
        self,
        pre_dict: PreDict = None,
        node: Node = None,
    ) -> dict[str, str] | None:
        """
        Generate dictionaries from the code parsed so far.

        This should be called after parsing something.

        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: generated params dictionary
        """
        pre_dict = pre_dict or PreDict()
        node = node or self.node

        if node in pre_dict.branch:
            depth = pre_dict.branch.index(node)
            # leaf nodes can only be reached once
            if not node.get_children():
                return None
        else:
            if pre_dict.branch and node not in pre_dict.branch[-1].get_children():
                raise ValueError("Discontinuous pre-dict branch, cannot get dicts")
            if not pre_dict.update_from_node(node):
                return None
            depth = len(pre_dict.branch) - 1

        # Reached leaf?
        if not node.get_children():
            self._debug("    reached leaf, returning it")
            d = pre_dict.get_dict()
            apply_suffix_bounds(d)
            return d

        # Recurse into children
        children = node.get_children()
        while True:
            if pre_dict.route[depth] is None:
                # start with 0th child
                pre_dict.route[depth] = 0
            elif (
                depth + 1 == len(pre_dict.route) - 1
            ):  # one for leaf down from final index
                # move to next child
                pre_dict.route[depth] += 1
                # remove all previous grand children and their effects on pre-dict
                for _ in range(depth + 1, len(pre_dict.route)):
                    pre_dict.reset_from_last_node()
            if pre_dict.route[depth] + 1 > len(children):
                break
            child = children[pre_dict.route[depth]]
            if (
                self.defaults
                and ".".join(str(node.var_name)) not in self.expand_defaults
            ):
                if any(c.default for c in children) and not child.default:
                    return None
            d = self.get_dicts_joined(pre_dict, child)
            # completed children recursion is consumed until we run out of children
            if d is None:
                continue
            return d
        return None

    def get_dicts_joined(
        self,
        pre_dict: PreDict = None,
        node: Node = None,
        skipdups: bool = True,
    ) -> dict[str, str] | None:
        """
        Get possibly joined dictionaries added using only filters.

        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: generated params dictionary

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

        if joins:
            if len(pre_dict.joins) == 0 or pre_dict.joins[-1] is None:

                # Rewrite all separate joins in one node as many `only'
                onlys = []
                for j in joins:
                    filename, linenum, obj = j
                    for word in obj.filter:
                        f = OnlyFilter([word], str(word))
                        onlys += [(filename, linenum, f)]

                # register the generator as leaf for current pre-dict and continue with a copy
                old_pre_dict = pre_dict.copy()
                pre_dict.update_from_node(node)
                pre_dict.joins[-1] = self.join_filters(
                    onlys, new_content, old_pre_dict, node
                )
            else:
                raise RuntimeError("Should not obtain joins if joins already expanded")

        # pre-existing generator has a next dictionary
        d = None
        if len(pre_dict.joins) > 0 and pre_dict.joins[-1] is not None:
            try:
                d = next(pre_dict.joins[-1])
            except StopIteration:
                pre_dict.route[-1] = len(node.get_children())

        if not joins and d is None:
            d = self.get_dicts_plain(pre_dict, node)
        return drop_suffixes(d, skipdups=skipdups) if d and parent else d

    def join_filters(
        self,
        onlys: list[tuple[str, int, Filter]],
        content_orig: list[list[tuple[str, int, "Token"]]],
        pre_dict_orig: PreDict,
        node: Node,
    ) -> Generator[dict[str, str], None, None]:
        """
        Perform all joins as filters on added dictionaries.

        :param onlys: list of only filters
        :param pre_dict: pre-dictionary of parsed content
        :param node: node to start from
        :returns: (recursive) dictionary generator

        Each `join' is the same as an `only' filter.
        """
        # Current join/only
        only = onlys[:1]
        remains = onlys[1:]

        node.swap_content(content_orig)
        for f, i, obj in only:
            node.add_content(f, i, obj)
        pre_dict = pre_dict_orig.copy()

        while True:
            d1 = self.get_dicts_plain(pre_dict, node)
            if d1 is None:
                break
            if not remains:
                yield d1
            else:

                # Current frame multiply by all variants from bottom
                for d2 in self.join_filters(remains, content_orig, pre_dict_orig, node):
                    d = d1.copy()
                    d.update(d2)
                    d["name"] = Node.join_names(d1["name"], d2["name"])
                    d["shortname"] = Node.join_names(d1["shortname"], d2["shortname"])
                    yield d

        node.swap_content(content_orig)
