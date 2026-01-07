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
        defaults: bool = False,
        expand_defaults: list[str] = None,
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
        self.joins: list[list[tuple[str, int, Filter] | None] | None] = []
        self.join_dicts: list[list[dict[str, str] | None] | None] = []
        self.join_pre_dicts: list[list["PreDict" | None] | None] = []

        # :param defaults: whether to use default variants
        self.defaults = defaults
        # :param expand_defaults: list of default variants to expand
        self.expand_defaults = expand_defaults or []

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
        new.join_dicts = self.join_dicts.copy()
        new.join_pre_dicts = self.join_pre_dicts.copy()

        return new

    def shallow_copy(self) -> "PreDict":
        new = PreDict()

        new._ctx[0] = self.ctx
        new._content[0] = self._content[-1]
        new._ctx_content[0] = self._ctx_content[-1]
        new._shortname[0] = self.shortname
        new._dep[0] = self.dep

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
        self.join_dicts += [None]
        self.join_pre_dicts += [None]

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
        self.join_dicts.pop()
        self.join_pre_dicts.pop()

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

    def get_dicts_plain(self) -> dict[str, str] | None:
        """
        Generate dictionaries from the pre-dict parsed so far.

        :returns: generated params dictionary

        This should be called after parsing something or seeding the
        pre-dict with an initial node.
        """
        if len(self.branch) == 0:
            raise RuntimeError("Pre-dictionary needs at least one node")
        depth = len(self.branch) - 1

        # Recurse into children
        while True:
            if depth < 0:
                break
            node = self.branch[depth]
            children = node.get_children()

            if self.route[depth] is None:
                # start with 0th child
                self.route[depth] = 0

                # Reached leaf?
                if not children:
                    # self._debug("    reached leaf, returning it")
                    d = self.get_dict()
                    apply_suffix_bounds(d)
                    return d

            elif depth + 1 == len(self.route) - 1:  # one for leaf down from final index
                # move to next child
                self.route[depth] += 1
                # remove all previous grand children and their effects on pre-dict
                for _ in range(depth + 1, len(self.route)):
                    self.reset_from_last_node()
            if self.route[depth] + 1 > len(children):
                depth -= 1
                continue

            child = children[self.route[depth]]
            if (
                self.defaults
                and ".".join(str(node.var_name)) not in self.expand_defaults
            ):
                if any(c.default for c in children) and not child.default:
                    return None
            d = self.get_dicts(child)
            # completed children recursion is consumed until we run out of children
            if d is None:
                # handle earlier reset of the same pre-dict by a nested getter
                depth = min(depth, len(self.branch) - 1)
                continue
            return d
        return None

    def get_dicts_joined(self) -> dict[str, str] | None:
        """
        Perform all joins as filters on added dictionaries.

        :returns: generated params dictionary

        Each `join' is the same as an `only' filter.
        """
        if len(self.branch) == 0:
            raise RuntimeError("Pre-dictionary needs at least one node")
        depth = len(self.branch) - 1
        node = self.branch[depth]
        joins = self.joins[depth]
        dicts = self.join_dicts[depth]
        pre_dicts = self.join_pre_dicts[depth]

        # join requires greedy dictionary expansion for variants of the same node
        width = 0
        while True:
            if width < 0:
                break
            if width < len(dicts) - 1 and dicts[width + 1]:
                width += 1
                continue

            # reset and update the pre-dict with differently filtered current node
            if pre_dicts[width] is None:
                # provide previous pre-dict clones to extend with a modified current node
                sub_pre_dict = self.copy()
                sub_pre_dict.reset_from_last_node()
                pre_dicts[width] = sub_pre_dict.shallow_copy()
            if node not in pre_dicts[width].branch:
                content_orig = node.get_content()
                # Current join/only
                node.add_content(*joins[width])
                if not pre_dicts[width].update_from_node(node):
                    return None
                node.swap_content(content_orig)

            dicts[width] = pre_dicts[width].get_dicts(node)
            if not dicts[width]:
                # remove all previous grand children and their effects on current pre-dict clone
                for _ in range(
                    len(pre_dicts[width].route) - 1, len(pre_dicts[width].route)
                ):
                    pre_dicts[width].reset_from_last_node()
                width -= 1
                continue

            # Current frame multiply by all variants from before
            if width == len(dicts) - 1:
                d = {}
                name, shortname = "", ""
                for di in dicts:
                    name = Node.join_names(name, di["name"]) if name else di["name"]
                    shortname = (
                        Node.join_names(shortname, di["shortname"])
                        if shortname
                        else di["shortname"]
                    )
                    d.update(di)
                d["name"], d["shortname"] = name, shortname
                return d

            width += 1

        return None

    def get_dicts(
        self,
        init_node: Node,
        dropsufs: bool = False,
        skipdups: bool = True,
    ) -> dict[str, str] | None:
        """
        Get possibly joined dictionaries added using only filters.

        :param init_node: node to start from
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
        if init_node not in self.branch:
            if self.branch and init_node not in self.branch[-1].get_children():
                raise ValueError("Discontinuous pre-dict branch, cannot get dicts")
            if not self.update_from_node(init_node):
                return None

        # due to pre-dict cloning current pre-dict must only contain one join at the end
        depth = len(self.branch) - 1
        node = self.branch[depth]
        joins = self.joins[depth]
        if joins is None:

            # Node is a current block. It has content, its contents: node.get_content()
            # Content without joins
            plain_content = []
            # All joins in current node
            joins = []
            for t in node.get_content():
                filename, linenum, obj = t
                if not isinstance(obj, JoinFilter):
                    plain_content.append(t)
                    continue
                # Accumulate all joins at one node
                joins += [t]

            # Rewrite all separate joins in one node as many `only'
            onlys = []
            for j in joins:
                filename, linenum, obj = j
                for word in obj.filter:
                    f = OnlyFilter([word], str(word))
                    onlys += [(filename, linenum, f)]

            joins = onlys
            if len(joins) > 0:
                # register join recursion as leaf for current pre-dict and continue with copies
                self.joins[-1] = joins
                self.join_dicts[-1] = [None for _ in joins]
                self.join_pre_dicts[-1] = [None for _ in joins]
                # provide join-free content to processed node
                node.swap_content(plain_content)

        d = None
        if joins:
            d = self.get_dicts_joined()
            if d is None:
                self.route[-1] = len(node.get_children())
        if d is None:
            d = self.get_dicts_plain()
        return drop_suffixes(d, skipdups=skipdups) if d and dropsufs else d


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

    def get_dicts_gen(
        self,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get dictionaries from a parser and given or its current node.

        :returns: (recursive) dictionary generator
        """
        pre_dict = PreDict(defaults=self.defaults)
        while True:
            # Since get_dicts() is recursive generator, it can invoke itself
            # and it can also be called outside to get dict generator.
            # Use special dropsufs argument to mark the top-level generator,
            # to be able to process all variables, do suffix stuff, drop dupes, etc.
            d = pre_dict.get_dicts(self.node, dropsufs=True, skipdups=skipdups)
            if d is None:
                break
            yield d
