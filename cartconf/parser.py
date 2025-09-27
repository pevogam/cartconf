"""
Module for readers, lexers, and parsers as well as their components.
"""

import os
import collections
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

Reader = lexer.Reader
Lexer = lexer.Lexer
LexerError = lexer.LexerError

Label = parser.Label


class Node(object):
    __slots__ = [
        "var_name",
        "name",
        "filename",
        "dep",
        "content",
        "children",
        "labels",
        "append_to_shortname",
        "failed_cases",
        "default",
    ]

    def __init__(self) -> None:
        self.var_name = []
        self.name = []
        self.filename = ""
        self.dep = []
        self.content = []
        self.children = []
        self.labels = set()
        self.append_to_shortname = False
        self.failed_cases = collections.deque()
        self.default = False

    def dump(self, indent: int, recurse: bool = False) -> str:
        """
        Dump node information as separate lines.

        :param indent: indentation level for the dump
        :param recurse: whether to recurse into child nodes
        :returns: string representation of the node data
        """
        dump_lines = [
            f"{' ' * indent}name: {self.name}",
            f"{' ' * indent}variable name: {self.var_name}",
            f"{' ' * indent}content: {self.content}",
            f"{' ' * indent}failed cases: {self.failed_cases}",
        ]
        if recurse:
            for child in self.children:
                dump_lines.append(child.dump(indent + 3, recurse))
        return "\n".join(dump_lines)


class ConditionalNode(Node):
    __slots__ = ["condition"]

    def __init__(self, condition: "Condition | NegativeCondition") -> None:
        super().__init__()
        self.condition = condition


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
        expand_defaults = expand_defaults or []
        self.expand_defaults = [LIdentifier(x) for x in expand_defaults]

        self.filename = filename
        if self.filename:
            self.parse_file(self.filename)

        self.only_filters = []
        self.no_filters = []
        self.assignments = []

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
        self.node = self._parse(Lexer(filename=cfgfile), self.node)
        self.filename = cfgfile

    def parse_string(self, cfgstr: str) -> None:
        """
        Parse a string.

        :param cfgstr: configuration string to parse
        """
        self.node.filename = Reader(content="").filename
        self.node = self._parse(Lexer(content=cfgstr), self.node)

    def only_filter(self, variant: str) -> None:
        """
        Apply a only filter programatically and keep track of it.

        Equivalent to parse a "only variant" line.

        :param variant: variant name to filter with
        """
        string = "only %s" % variant
        self.only_filters.append(string)
        self.parse_string(string)

    def no_filter(self, variant: str) -> None:
        """
        Apply a no filter programatically and keep track of it.

        Equivalent to parse a "no variant" line.

        :param variant: variant name to filter with
        """
        string = "no %s" % variant
        self.no_filters.append(string)
        self.parse_string(string)

    def assign(self, key: str, value: str) -> None:
        """
        Apply an assignment programatically and keep track of it.

        Equivalent to parse a "key = value" line.

        :param key: key to assign to
        :param value: value to assign
        """
        string = "%s = %s" % (key, value)
        self.assignments.append(string)
        self.parse_string(string)

    @staticmethod
    def parse_filter(
        lexer: Lexer, tokens: list["Token"]
    ) -> "list[list[Label | Token]]":
        """
        Parse a filter from a list of tokens.

        :param lexer: lexer to use for parsing
        :param tokens: list of tokens to parse
        :returns: parsed filters
        :raises: :py:class:`ParserError` if the syntax contains errors

        More details on the syntax of the connectives for these filters:

        * ``,`` means ``OR``
        * ``..`` means ``AND``
        * ``.`` means ``IMMEDIATELY-FOLLOWED-BY``
        * ``(xx=yy)`` where ``xx=VARIANT_NAME`` and ``yy=VARIANT_VALUE``

        Example:

        ::

            qcow2..(guest_os=Fedora).14, RHEL.6..raw..boot, smp2..qcow2..migrate..ide

        means match all dicts whose names have:

        ::

            (qcow2 AND ((guest_os=Fedora) IMMEDIATELY-FOLLOWED-BY 14)) OR
            ((RHEL IMMEDIATELY-FOLLOWED-BY 6) AND raw AND boot) OR
            (smp2 AND qcow2 AND migrate AND ide)

        Note:

        * ``qcow2..Fedora.14`` is equivalent to ``Fedora.14..qcow2``.
        * ``qcow2..Fedora.14`` is not equivalent to ``qcow2..14.Fedora``.
        * ``ide, scsi`` is equivalent to ``scsi, ide``.
        """
        or_filters = []
        tokens = iter(tokens + [LEndL()])
        and_filter = []
        con_filter = []
        dots = 1

        def check_token(
            token: "Token", allowed_tokens: list[type]
        ) -> tuple[type, "Token"]:
            lexer.check_token(token, allowed_tokens)
            return type(token), token

        def next_nw(gen: Generator["Token", None, None]) -> "Token":
            token = next(gen)
            while isinstance(token, LWhite):
                token = next(gen)
            return token

        typet, token = check_token(
            next(tokens), [LIdentifier, LLRBracket, LEndL, LWhite]
        )
        while typet not in [LEndL]:
            if typet in [LIdentifier, LLRBracket]:  # join    identifier
                if typet == LLRBracket:  # (xxx=ttt)
                    _, ident = check_token(next_nw(tokens), [LIdentifier])  # (iden
                    typet, _ = check_token(next_nw(tokens), [LSet, LRRBracket])  # =
                    if typet == LRRBracket:  # (xxx)
                        token = Label(ident.string)
                    elif typet == LSet:  # (xxx = yyyy)
                        _, value = check_token(next_nw(tokens), [LIdentifier, LString])
                        check_token(next_nw(tokens), [LRRBracket])
                        token = Label(ident.string, value.string)
                else:
                    token = Label(token.string)
                if dots == 1:
                    con_filter.append(token)
                elif dots == 2:
                    and_filter.append(con_filter)
                    con_filter = [token]
                elif dots == 0 or dots > 2:
                    raise ParserError(
                        'Syntax Error expected "." between' " Identifier.",
                        lexer.line,
                        lexer.filename,
                        lexer.linenum,
                    )

                dots = 0
            elif typet == LDot:  # xxx.xxxx or xxx..xxxx
                dots += 1
            elif typet in [LComa, LWhite]:
                if dots > 0:
                    raise ParserError(
                        "Syntax Error expected identifier between" ' "." and ",".',
                        lexer.line,
                        lexer.filename,
                        lexer.linenum,
                    )
                if and_filter:
                    if con_filter:
                        and_filter.append(con_filter)
                        con_filter = []
                    or_filters.append(and_filter)
                    and_filter = []
                elif con_filter:
                    or_filters.append([con_filter])
                    con_filter = []
                elif typet == LIdentifier:
                    or_filters.append([[Label(token.string)]])
                else:
                    raise ParserError(
                        'Syntax Error expected "," between' " Identifier.",
                        lexer.line,
                        lexer.filename,
                        lexer.linenum,
                    )
                dots = 1
                token = next(tokens)
                while isinstance(token, LWhite):
                    token = next(tokens)
                typet, token = check_token(
                    token, [LIdentifier, LComa, LDot, LLRBracket, LEndL]
                )
                continue
            typet, token = check_token(
                next(tokens), [LIdentifier, LComa, LDot, LLRBracket, LEndL, LWhite]
            )
        if and_filter:
            if con_filter:
                and_filter.append(con_filter)
                con_filter = []
            or_filters.append(and_filter)
            and_filter = []
        if con_filter:
            or_filters.append([con_filter])
            con_filter = []
        return or_filters

    @staticmethod
    def _cmd_tokens(tokens1: list["Token"], tokens2: list["Token"]) -> bool:
        for x, y in list(zip(tokens1, tokens2)):
            if x != y:
                return False
        else:
            return True

    @staticmethod
    def _apply_predict(
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
    ) -> None:
        predict = LApplyPreDict("", pre_dict.copy())
        node.content += [(lexer.filename, lexer.linenum, predict)]
        pre_dict.clear()

    def _apply_include(
        self,
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
    ) -> Node:
        """
        Parse:
           include relative file patch to working directory.
        """
        path = lexer.get_rest_line_as_string_token()
        filename = os.path.expanduser(path.string)
        if lexer.filename != "<string>" and not os.path.isabs(filename):
            filename = os.path.join(os.path.dirname(lexer.filename), filename)
        if not os.path.isfile(filename):
            raise MissingIncludeError(lexer.line, lexer.filename, lexer.linenum)
        Parser._apply_predict(lexer, node, pre_dict)
        lch = Lexer(filename=filename)
        node = self._parse(lch, node, -1)
        return node

    @staticmethod
    def _apply_operator(
        identifier: list["Token"],
        token: "Token",
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
    ) -> None:
        """
        Parse:
           identifier = xxx
           identifier <= xxx
           identifier ?= xxx
           etc..
        """
        op = identifier[-1]
        if len(identifier) == 1:
            identifier_str = token.string
        else:
            identifier = [token] + identifier[:-1]
            identifier_str = "".join([x.string for x in identifier])
        value = lexer.get_next_token([LString])
        value_str = value.string
        if value_str and (
            value_str[0] == value_str[-1] == '"' or value_str[0] == value_str[-1] == "'"
        ):
            value_str = value_str[1:-1]

        op = type(op)(identifier_str, value_str)
        d_nin_val = "$" not in value_str
        if isinstance(op, LSet) and d_nin_val:  # Optimization
            op.apply_to_dict(pre_dict)
        else:
            if pre_dict:
                # Flush pre_dict to node content.
                # If block already contains xxx = yyyy
                # then the operations xxx +=, <=, .... are safe.
                if op.name in pre_dict and d_nin_val:
                    op.apply_to_dict(pre_dict)
                    lexer.get_next_token([LEndL])
                    return
                else:
                    Parser._apply_predict(lexer, node, pre_dict)
            node.content += [(lexer.filename, lexer.linenum, op)]
        lexer.get_next_token([LEndL])

    def _apply_deletion(
        self,
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
    ) -> None:
        """
        Parse:
            del operand
        """
        to_del = lexer.get_next_token([LIdentifier], no_white=True)
        lexer.get_next_token([LEndL], no_white=True)
        token = LDel(to_del.string, "")

        Parser._apply_predict(lexer, node, pre_dict)
        node.content += [(lexer.filename, lexer.linenum, token)]

    def _apply_condition(
        self,
        identifier: list["Token"],
        token: "Token",
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
        indent: int,
    ) -> None:
        """
        Parse:
           xxx.yyy.(aaa=bbb):
        """
        identifier = [token] + identifier[:-1]
        cfilter = Parser.parse_filter(lexer, identifier + [LEndL()])
        next_line = lexer.get_rest_line_as_string_token()
        if next_line.string != "":
            lexer.set_next_line(next_line.string, indent + 1, lexer.linenum)
        cond = ConditionalNode(Condition(cfilter, lexer.line))
        self._parse(lexer, cond, prev_indent=indent)

        Parser._apply_predict(lexer, node, pre_dict)
        node.content += [(lexer.filename, lexer.linenum, cond)]

    def _apply_notcondition(
        self,
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
        indent: int,
    ) -> None:
        """
        Parse:
           !xxx.yyy.(aaa=bbb): vvv
        """
        lfilter = Parser.parse_filter(
            lexer, lexer.get_until([LColon, LEndL], no_white=True)[:-1]
        )
        next_line = lexer.get_rest_line_as_string_token()
        if next_line.string != "":
            lexer.set_next_line(next_line.string, indent + 1, lexer.linenum)
        cond = ConditionalNode(NegativeCondition(lfilter, lexer.line))
        self._parse(lexer, cond, prev_indent=indent)

        Parser._apply_predict(lexer, node, pre_dict)
        node.content += [(lexer.filename, lexer.linenum, cond)]

    @staticmethod
    def _apply_variants(
        lexer: Lexer,
        node: Node,
    ) -> tuple[str, dict[str, str]]:
        """
        Parse:
           variants _name_ [meta1] [meta2]:
        """
        if type(node) is ConditionalNode:
            raise ParserError(
                "'variants' is not allowed inside a " "conditional block",
                lexer.line,
                lexer.filename,
                lexer.linenum,
            )

        tokens = lexer.get_until([LLBracket, LColon, LIdentifier, LEndL], no_white=True)
        vtypet = type(tokens[-1])
        variant_name = ""
        meta = {}
        # [meta1=xxx] [yyy] [xxx]
        while vtypet not in [LColon, LEndL]:
            if vtypet == LIdentifier:
                if variant_name != "":
                    raise ParserError(
                        "Syntax ERROR expected" ' "[" or ":"',
                        lexer.line,
                        lexer.filename,
                        lexer.linenum,
                    )
                variant_name = tokens[0].string
            elif vtypet == LLBracket:  # [
                ident = lexer.get_next_token([LIdentifier], no_white=True)
                typet = type(lexer.get_next_token([LSet, LRBracket], no_white=True))
                if typet == LRBracket:  # [xxx]
                    if ident.string not in meta:
                        meta[ident.string] = []
                    meta[ident.string].append(True)
                elif typet == LSet:  # [xxx = yyyy]
                    tokens = lexer.get_until([LRBracket, LEndL], no_white=True)
                    if isinstance(tokens[-1], LRBracket):
                        if ident.string not in meta:
                            meta[ident.string] = []
                        meta[ident.string].append(tokens[:-1])
                    else:
                        raise ParserError(
                            "Syntax ERROR" ' expected "]"',
                            lexer.line,
                            lexer.filename,
                            lexer.linenum,
                        )

            varianst_allowed_in = [LLBracket, LColon, LIdentifier, LEndL]
            vtypet = type(lexer.get_next_token(varianst_allowed_in, no_white=True))

        if "default" in meta:
            for wd in meta["default"]:
                if not isinstance(wd, list):
                    raise ParserError(
                        "Syntax ERROR expected " "[default=xxx]",
                        lexer.line,
                        lexer.filename,
                        lexer.linenum,
                    )

        if vtypet == LEndL:
            raise ParserError(
                'Syntax ERROR expected ":"',
                lexer.line,
                lexer.filename,
                lexer.linenum,
            )
        lexer.get_next_token([LEndL], no_white=True)

        return variant_name, meta

    def _apply_variant(
        self,
        token: "Token",
        lexer: Lexer,
        node: Node,
        pre_dict: dict[str, str],
        indent: int,
        variant_name: str,
        variant_indent: int,
        meta: dict[str, str],
    ) -> Node:
        """
        Parse:
         - var1: depend1, depend2
             block1
         - var2:
             block2
        """
        if pre_dict:
            Parser._apply_predict(lexer, node, pre_dict)
        already_default = False
        is_default = False
        meta_with_default = False
        if "default" in meta:
            meta_with_default = True
        meta_in_expand_defautls = False
        if variant_name not in self.expand_defaults:
            meta_in_expand_defautls = True
        node4 = Node()
        while True:
            lexer.set_prev_indent(variant_indent)
            # Get token from lexer and check syntax.
            token = lexer.get_next_token(
                [LIdentifier, LDefault, LIndent, LEndBlock],
                no_white=True,
            )
            typet = type(token)
            if typet == LEndBlock:
                break

            if typet == LIndent:
                lexer.get_next_token([LVariant], no_white=True)
                token = lexer.get_next_token([LIdentifier, LDefault], no_white=True)
                typet = type(token)

            if typet == LDefault:  # @
                is_default = True
                name = lexer.get_until([LColon], [LIdentifier, LDot])
            else:  # identificator
                is_default = False
                name = [token] + lexer.get_until([LColon], [LIdentifier, LDot])

            if len(name) == 2:
                raw_name = name
                name = [name[0].string]
            else:
                raw_name = [x for x in name[:-1]]
                name = [x.string for x in name[:-1] if isinstance(x, LIdentifier)]

            token = lexer.get_next_token()
            while isinstance(token, LWhite):
                token = lexer.get_next_token()
            tokens = None
            if not isinstance(token, LEndL):
                tokens = [token] + lexer.get_until([LEndL])
                deps = Parser.parse_filter(lexer, tokens)
            else:
                deps = []

            # Prepare data for dict generator.
            node2 = Node()
            node2.children = [node]
            node2.labels = node.labels

            if variant_name:
                op = LSet(variant_name, ".".join([n for n in name]))
                node2.content += [(lexer.filename, lexer.linenum, op)]

            node3 = self._parse(lexer, node2, prev_indent=indent)

            if variant_name:
                node3.var_name = variant_name
                node3.name = [Label(variant_name, n) for n in name]
            else:
                node3.name = [Label(n) for n in name]

            # Update mapping name to file

            node3.dep = deps

            if meta_with_default:
                for wd in meta["default"]:
                    if Parser._cmd_tokens(wd, raw_name):
                        is_default = True
                        meta["default"].remove(wd)

            if is_default and not already_default and meta_in_expand_defautls:
                node3.default = True
                already_default = True

            node3.append_to_shortname = not is_default

            op = LUpdateFileMap(
                lexer.filename,
                ".".join(str(x) for x in node3.name),
                "_name_map_file",
            )
            node3.content += [(lexer.filename, lexer.linenum, op)]

            op = LUpdateFileMap(
                lexer.filename,
                ".".join(str(x.name) for x in node3.name),
                "_short_name_map_file",
            )
            node3.content += [(lexer.filename, lexer.linenum, op)]

            if node3.default and self.defaults:
                # Move default variant in front of rest
                # of all variants.
                # Speed optimization.
                node4.children.insert(0, node3)
            else:
                node4.children += [node3]
            node4.labels.update(node3.labels)
            node4.labels.update(node3.name)

        if "default" in meta and meta["default"]:
            raise ParserError(
                "Missing default variant %s" % (meta["default"]),
                lexer.line,
                lexer.filename,
                lexer.linenum,
            )
        return node4

    def _parse(self, lexer: Lexer, node: Node = None, prev_indent: int = -1) -> Node:
        if not node:
            node = self.node

        block_allowed = [
            LVariants,
            LIdentifier,
            LOnly,
            LNo,
            LInclude,
            LDel,
            LNotCond,
            LSuffix,
            LJoin,
        ]
        variants_allowed = [LVariant]
        identifier_allowed = [
            LSet,
            LAppend,
            LPrepend,
            LLazySet,
            LRegExpSet,
            LRegExpAppend,
            LRegExpPrepend,
            LColon,
            LEndL,
        ]
        indent_allowed = [LIndent, LEndBlock]
        allowed = block_allowed

        # variant name and indent
        variant_name = ""
        variant_indent = 0
        # meta contains variants meta-data
        meta = {}
        # pre_dict contains block of operation without collision with
        # others block or operation. Increase speed almost twice.
        pre_dict = {}

        # Suffix should be applied as the last operator in the dictionary
        # Reasons:
        #     1. Escape multiply suffix operators
        #     2. Affect all elements in current block
        suffix = None

        try:
            while True:
                lexer.set_prev_indent(prev_indent)
                token = lexer.get_next_token(indent_allowed)
                typet = type(token)
                if typet == LEndBlock:
                    if pre_dict:
                        # flush pre_dict to node content.
                        Parser._apply_predict(lexer, node, pre_dict)
                    if suffix:
                        # Node has suffix, apply it to all elements
                        node.content.append(suffix)
                    return node

                indent = token.length
                token = lexer.get_next_token(allowed)
                typet = type(token)

                if typet == LInclude:
                    node = self._apply_include(lexer, node, pre_dict)
                    lexer.set_prev_indent(prev_indent)

                elif typet == LIdentifier:
                    # Parse:
                    #    identifier .....
                    identifier = lexer.get_until(identifier_allowed, no_white=True)
                    if tokens_oper_key(identifier[-1]) in list(
                        tokens_oper
                    ):  # operand = <=
                        Parser._apply_operator(identifier, token, lexer, node, pre_dict)
                    elif isinstance(identifier[-1], LColon):  # condition:
                        self._apply_condition(
                            identifier, token, lexer, node, pre_dict, indent
                        )
                    else:
                        raise ParserError(
                            'Syntax ERROR expected ":" or' " operand",
                            lexer.line,
                            lexer.filename,
                            lexer.linenum,
                        )
                elif typet == LDel:
                    self._apply_deletion(lexer, node, pre_dict)
                elif typet == LNotCond:
                    self._apply_notcondition(lexer, node, pre_dict, indent)
                    lexer.set_prev_indent(prev_indent)

                elif typet == LVariants:  # _name_ [meta1=xxx] [yyy] [xxx]
                    variant_name, meta = Parser._apply_variants(lexer, node)
                    variant_indent = indent
                    allowed = variants_allowed
                elif typet == LVariant:
                    node = self._apply_variant(
                        token,
                        lexer,
                        node,
                        pre_dict,
                        indent,
                        variant_name,
                        variant_indent,
                        meta,
                    )
                    allowed = block_allowed

                elif typet in [LNo, LOnly, LJoin]:
                    # Parse:
                    #    only/no/join (filter=text)..aaa.bbb, xxxx
                    lfilter = Parser.parse_filter(lexer, lexer.get_rest_line())
                    Parser._apply_predict(lexer, node, pre_dict)
                    if typet == LOnly:
                        node.content += [
                            (
                                lexer.filename,
                                lexer.linenum,
                                OnlyFilter(lfilter, lexer.line),
                            )
                        ]
                    elif typet == LNo:
                        node.content += [
                            (
                                lexer.filename,
                                lexer.linenum,
                                NoFilter(lfilter, lexer.line),
                            )
                        ]
                    else:  # LJoin
                        node.content += [
                            (
                                lexer.filename,
                                lexer.linenum,
                                JoinFilter(lfilter, lexer.line),
                            )
                        ]

                elif typet == LSuffix:
                    # Parse:
                    #    suffix SUFFIX
                    if pre_dict:
                        Parser._apply_predict(lexer, node, pre_dict)
                    token_val = lexer.get_next_token([LIdentifier])
                    lexer.get_next_token([LEndL])
                    suffix_operator = Suffix("", token_val.string)
                    # Suffix will be applied as all other elements in current node are processed:
                    suffix = (
                        lexer.filename,
                        lexer.linenum,
                        suffix_operator,
                    )

                else:
                    raise ParserError(
                        "Syntax ERROR expected",
                        lexer.line,
                        lexer.filename,
                        lexer.linenum,
                    )
        except Exception:
            self._debug("%s  %s:  %s" % (lexer.filename, lexer.linenum, lexer.line))
            raise

    def get_dicts(
        self,
        node: Node = None,
        ctx: list[list[Label]] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[str] = None,
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
        ctx: list[list[Label]] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[str] = None,
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
                filter = obj.condition if type(obj) is ConditionalNode else obj
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
                            failed_filters.append(t)
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
                        if not process_content(obj.content, new_internal_filters):
                            failed_filters.append(t)
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
            all_content = content + node.content
            for t in failed_external_filters + failed_internal_filters:
                if t not in all_content:
                    return True
            for t in failed_external_filters:
                _, _, external_filter = t
                if not external_filter.might_pass(failed_ctx, ctx, labels):
                    return False
            for t in failed_internal_filters:
                if t not in node.content:
                    return True

            for t in failed_internal_filters:
                _, _, internal_filter = t
                if not internal_filter.might_pass(failed_ctx, ctx, labels):
                    return False
            return True

        def add_failed_case():
            node.failed_cases.appendleft(
                (ctx, new_external_filters, new_internal_filters)
            )
            if len(node.failed_cases) > Parser.num_failed_cases:
                node.failed_cases.pop()

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
        for i, failed_case in enumerate(node.failed_cases):
            if not might_pass(*failed_case):
                self._debug(
                    "\n*    this subtree has failed before %s\n"
                    "         content: %s\n"
                    "         failcase:%s\n",
                    name,
                    content + node.content,
                    failed_case,
                )
                del node.failed_cases[i]
                node.failed_cases.appendleft(failed_case)
                return

        # Check content and unpack it into new_content
        new_content = []
        new_external_filters = []
        new_internal_filters = []
        if not process_content(
            node.content, new_internal_filters
        ) or not process_content(content, new_external_filters):
            add_failed_case()
            self._debug("Failed_cases %s", node.failed_cases)
            return

        # Update shortname
        if node.append_to_shortname:
            shortname = shortname + node.name

        # Recurse into children
        count = 0
        if self.defaults and node.var_name not in self.expand_defaults:
            for n in node.children:
                for d in self.get_dicts_joined(n, ctx, new_content, shortname, dep):
                    count += 1
                    yield d
                if n.default and count:
                    break
        else:
            for n in node.children:
                for d in self.get_dicts_joined(n, ctx, new_content, shortname, dep):
                    count += 1
                    yield d
        # Reached leaf?
        if not node.children:
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
        ctx: list[list[Label]] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[str] = None,
        dep: list[str] = None,
        skipdups: bool = True,
    ) -> Generator[dict[str, str], None, None]:
        """
        Get possibly joined dictionaries added using only filters.

        :param node: node to start from
        :param ctx: node labels/names
        :param content: previous content in plain
        :param shortname: short name
        :param dep: dependmake_nameencies
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

        # Node is a current block. It has content, its contents: node.content
        # Content without joins
        new_content = []

        # All joins in current node
        joins = []

        for t in node.content:
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

            old_content = node.content[:]
            node.content = new_content
            for d in self.join_filters(onlys, node, ctx, content, shortname, dep):
                yield drop_suffixes(d, skipdups=skipdups) if parent else d
            node.content = old_content[:]

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
        ctx: list[list[Label]] = None,
        content: list[tuple[str, int, "Token"]] = None,
        shortname: list[str] = None,
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

        content_orig = node.content[:]
        node.content += only

        if not remains:
            for d in self.get_dicts_plain(node, ctx, content, shortname, dep):
                yield d
        else:
            for d1 in self.get_dicts_plain(node, ctx, content, shortname, dep):
                # Current frame multiply by all variants from bottom
                node.content = content_orig
                for d2 in self.join_filters(
                    remains, node, ctx, content, shortname, dep
                ):

                    d = d1.copy()
                    d.update(d2)
                    d["name"] = self.join_names(d1["name"], d2["name"])
                    d["shortname"] = self.join_names(d1["shortname"], d2["shortname"])
                    yield d
