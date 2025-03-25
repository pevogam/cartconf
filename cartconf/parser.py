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


LOG = logging.getLogger("avocado." + __name__)


class Label(object):
    __slots__ = ["name", "var_name", "long_name", "hash_val", "hash_var"]

    def __init__(self, name: str, next_name: str = None) -> None:
        self.name = next_name if next_name else name
        self.var_name = name if next_name else None
        self.long_name = (
            f"({self.var_name}={self.name})" if self.var_name else f"{self.name}"
        )
        self.hash_val = self.hash_name()
        self.hash_var = self.hash_variant() if self.var_name else None

    def __str__(self) -> str:
        return self.long_name

    def __repr__(self) -> str:
        return self.long_name

    def __eq__(self, o: "Label") -> bool:
        """The comparison is asymmetric due to optimization."""
        return self.long_name == o.long_name if o.var_name else self.name == o.name

    def __ne__(self, o: "Label") -> bool:
        """The comparison is asymmetric due to optimization."""
        return self.long_name != o.long_name if o.var_name else self.name != o.name

    def __hash__(self) -> int:
        return self.hash_val

    def hash_name(self) -> int:
        return sum((i + 1) * ord(x) for i, x in enumerate(self.name))

    def hash_variant(self) -> int:
        return sum((i + 1) * ord(x) for i, x in enumerate(str(self)))


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
        dump_str = f"{' ' * indent}name: {self.name}\n"
        dump_str += f"{' ' * indent}variable name: {self.var_name}\n"
        dump_str += f"{' ' * indent}content: {self.content}\n"
        dump_str += f"{' ' * indent}failed cases: {self.failed_cases}\n"
        if recurse:
            for child in self.children:
                dump_str += child.dump(indent + 3, recurse)
        return dump_str


class StrReader(object):
    """
    Preprocess an input string for easy reading.
    """

    def __init__(self, s: str) -> None:
        """
        Initialize the reader.

        :param s: The string to parse.
        """
        self.filename = "<string>"
        self._lines = []
        self._line_index = 0
        self._stored_line = None
        for linenum, line in enumerate(s.splitlines()):
            line = line.rstrip().expandtabs()
            stripped_line = line.lstrip()
            indent = len(line) - len(stripped_line)
            if not stripped_line or stripped_line.startswith(("#", "//")):
                continue
            self._lines.append((stripped_line, indent, linenum + 1))

    def get_next_line(self, prev_indent: int) -> tuple[str | None, int, int]:
        """
        Get the next line in the current block.

        :param prev_indent: The indentation level of the previous block.
        :returns: (line, indent, linenum), where indent is the line's
            indentation level.  If no line is available, (None, -1, -1) is
            returned.
        """
        if self._stored_line:
            ret = self._stored_line
            self._stored_line = None
            return ret
        if self._line_index >= len(self._lines):
            return None, -1, -1
        line, indent, linenum = self._lines[self._line_index]
        if indent <= prev_indent:
            return None, indent, linenum
        self._line_index += 1
        return line, indent, linenum

    def set_next_line(self, line: str, indent: int, linenum: int) -> None:
        """
        Make the next call to get_next_line() return the given line instead of
        the real next line.
        """
        line = line.strip()
        if line:
            self._stored_line = line, indent, linenum


class FileReader(StrReader):
    """
    Preprocess an input file for easy reading.
    """

    def __init__(self, filename: str) -> None:
        """
        Initialize the reader.

        :param filename: name of the input file
        """
        with open(filename) as f:
            super().__init__(f.read())
        self.filename = filename


class Lexer(object):

    tokens_oper_re = [r"\=", r"\+\=", r"\<\=", r"\~\=", r"\?\=", r"\?\+\=", r"\?\<\="]
    _ops_exp = re.compile(r"|".join(tokens_oper_re))
    spec_iden = "_-"
    spec_oper = "+<?~"

    def __init__(self, reader: StrReader | FileReader) -> None:
        """
        Initialize the lexer.

        :param reader: file or string reader to get lines from
        """
        self.reader = reader
        self.filename = reader.filename
        self.line = None
        self.linenum = 0
        self.ignore_white = False
        self.rest_as_string = False
        self.match_func_index = 0
        self.generator = self.get_lexer()
        self.prev_indent = 0
        self.fast = False

    def set_prev_indent(self, prev_indent: int) -> None:
        self.prev_indent = prev_indent

    def set_fast(self) -> None:
        self.fast = True

    def set_strict(self) -> None:
        self.fast = False

    def match(self, line: str, pos: int) -> Generator[Token, None, None]:
        """
        Generate tokens from a string line in order of matching.

        :param line: line to parse from
        :param pos: position in the line to start parsing from
        :returns: iterator of tokens that were read
        :raises: :py:class:`LexerError` if unexpected character is found
        """
        l0 = line[0]
        chars = ""
        m = None
        cind = 0
        if l0 == "v":
            if line.startswith("variants:"):
                yield LVariants()
                yield LColon()
                pos = 9
            elif line.startswith("variants "):
                yield LVariants()
                pos = 8
        elif l0 == "-":
            yield LVariant()
            pos = 1
        elif l0 == "o":
            if line.startswith("only "):
                yield LOnly()
                pos = 4
                while line[pos].isspace():
                    pos += 1
        elif l0 == "n":
            if line.startswith("no "):
                yield LNo()
                pos = 2
                while line[pos].isspace():
                    pos += 1
        elif l0 == "i":
            if line.startswith("include "):
                yield LInclude()
                pos = 7
        elif l0 == "d":
            if line.startswith("del "):
                yield LDel()
                pos = 3
                while line[pos].isspace():
                    pos += 1
        elif l0 == "s":
            if line.startswith("suffix "):
                yield LSuffix()
                pos = 6
                while line[pos].isspace():
                    pos += 1
        elif l0 == "j":
            if line.startswith("join "):
                yield LJoin()
                pos = 4
                while line[pos].isspace():
                    pos += 1

        if self.fast and pos == 0:  # due to refexp
            cind = line[pos:].find(":")
            m = Lexer._ops_exp.search(line[pos:])

        oper = ""
        token = None

        if self.rest_as_string:
            self.rest_as_string = False
            yield LString(line[pos:].lstrip())
        elif self.fast and m and (cind < 0 or cind > m.end()):
            chars = ""
            yield LIdentifier(line[: m.start()].rstrip())
            yield tokens_oper[m.group()[:-1]]()
            yield LString(line[m.end() :].lstrip())
        else:
            li = enumerate(line[pos:], pos)
            for pos, char in li:
                if char.isalnum() or char in Lexer.spec_iden:  # alfanum+_-
                    chars += char
                elif char in Lexer.spec_oper:  # <+?=~
                    if chars:
                        yield LIdentifier(chars)
                        oper = ""
                    chars = ""
                    oper += char
                else:
                    if chars:
                        yield LIdentifier(chars)
                        chars = ""
                    if char.isspace():  # Whitespace
                        for pos, char in li:
                            if not char.isspace():
                                if not self.ignore_white:
                                    yield LWhite()
                                break
                    if char.isalnum() or char in Lexer.spec_iden:
                        chars += char
                    elif char == "=":
                        if oper in tokens_oper:
                            yield tokens_oper[oper]()
                        else:
                            raise LexerError(
                                "Unexpected character %s on" " pos %s" % (char, pos),
                                self.line,
                                self.filename,
                                self.linenum,
                            )
                        oper = ""
                    elif char in tokens_map:
                        token = tokens_map[char]()
                    elif char == '"':
                        chars = ""
                        pos, char = next(li)
                        while char != '"':
                            chars += char
                            pos, char = next(li)
                        yield LString(chars)
                    elif char == "#":
                        break
                    elif char in Lexer.spec_oper:
                        oper += char
                    else:
                        raise LexerError(
                            "Unexpected character %s on"
                            " pos %s. Special chars are allowed"
                            " only in variable assignation"
                            " statement" % (char, pos),
                            line,
                            self.filename,
                            self.linenum,
                        )
                    if token is not None:
                        yield token
                        token = None
                    if self.rest_as_string:
                        self.rest_as_string = False
                        yield LString(line[pos + 1 :].lstrip())
                        break
        if chars:
            yield LIdentifier(chars)
            chars = ""
        yield LEndL()

    def get_lexer(self) -> Generator[Token, None, None]:
        """
        Generate tokens from a multi-line reader in order of matching.

        :returns: iterator of tokens that were read

        ..warning:: This generator will never terminate and needs checks for end tokens.
        """
        cr = self.reader
        indent = 0
        while True:
            (self.line, indent, self.linenum) = cr.get_next_line(self.prev_indent)

            if not self.line:
                yield LEndBlock(indent)
                continue

            yield LIndent(indent)
            for token in self.match(self.line, 0):
                yield token

    def get_until_gen(
        self, end_tokens: list[type] = None
    ) -> Generator[Token, None, None]:
        """
        Generate tokens from a multi-line reader terminating at a list of end tokens.

        :param end_tokens: list of tokens to terminate reading on with default end-of-line token
        :returns: iterator of tokens that were read
        """
        end_tokens = end_tokens or [LEndL]
        token = next(self.generator)
        while type(token) not in end_tokens:
            yield token
            token = next(self.generator)
        yield token

    def get_until(self, end_tokens: list[type] = None) -> list[Token]:
        """
        Get a full list of tokens from a multi-line reader terminating at a list of end tokens.

        :param end_tokens: list of tokens to terminate reading on with default end-of-line token
        :returns: list of tokens that were read
        """
        end_tokens = end_tokens or [LEndL]
        return [x for x in self.get_until_gen(end_tokens)]

    def flush_until(self, end_tokens: list[type] = None) -> None:
        """
        Skip all tokens until any in a list of end tokens.

        :param end_tokens: list of tokens to terminate reading on with default end-of-line token
        """
        end_tokens = end_tokens or [LEndL]
        for _ in self.get_until_gen(end_tokens):
            pass

    def get_until_check(
        self, allowed_tokens: list[type], end_tokens: list[type] = None
    ) -> list[Token]:
        """
        Get a full list of tokens from acceptable ones terminating at a list of end ones.

        :param allowed_tokens: list of allowed tokens
        :param end_tokens: list of tokens to terminate reading on with default end-of-line token
        :returns: list of tokens that were read
        :raises: :py:class:`ParserError` if unexpected token is found
        """
        end_tokens = end_tokens or [LEndL]
        tokens = []
        allowed_tokens = allowed_tokens + end_tokens
        for token in self.get_until_gen(end_tokens):
            if type(token) in allowed_tokens:
                tokens.append(token)
            else:
                raise ParserError(
                    "Expected %s got %s" % (allowed_tokens, type(token)),
                    self.line,
                    self.filename,
                    self.linenum,
                )
        return tokens

    def get_until_no_white(self, end_tokens: list[type] = None) -> list[Token]:
        """
        Get a full list of tokens terminating at a list of end tokens and strip white space ones.

        :param end_tokens: list of tokens to terminate reading on with default end-of-line token
        :returns: list of tokens that were read
        """
        end_tokens = end_tokens or [LEndL]
        return [x for x in self.get_until_gen(end_tokens) if not isinstance(x, LWhite)]

    def rest_line_gen(self) -> Generator[Token, None, None]:
        """
        Generate tokens from the rest of the line terminating only at an end-of-line token.
        :returns: iterator of tokens that were read

        :returns: iterator of tokens that were read
        """
        token = next(self.generator)
        while not isinstance(token, LEndL):
            yield token
            token = next(self.generator)

    def rest_line(self) -> list[Token]:
        """
        Get a full list of tokens from the rest of the line terminating only at an end-of-line token.

        :returns: list of tokens that were read
        """
        return [x for x in self.rest_line_gen()]

    def rest_line_no_white(self) -> list[Token]:
        """
        Get a full list of tokens from the rest of the line and strip white space ones.

        :returns: list of tokens that were read
        """
        return [x for x in self.rest_line_gen() if not isinstance(x, LWhite)]

    def rest_line_as_string_token(self) -> LString:
        """
        Get a string token from the rest of the line.

        :returns: rest of the line as a string token
        :raises: :py:class:`ParserError` if the remaining token is not a string token
            followed by an end-of-line token
        """
        self.rest_as_string = True
        remainder_string = next(self.generator)
        if type(remainder_string) is not LString:
            raise ParserError("Expected string, got %s" % type(remainder_string))
        # skip the end-of-line token
        end_of_line = next(self.generator)
        if type(end_of_line) is not LEndL:
            raise ParserError("Expected end-of-line, got %s" % type(end_of_line))
        return remainder_string

    def get_next_check(self, allowed_tokens: list[type]) -> tuple[type, Token]:
        """
        Get the next token and throw an error if it is not acceptable.

        :param allowed_tokens: list of allowed tokens
        :returns: the next acceptable token and its type
        :raises: :py:class:`ParserError` if token is not acceptable
        """
        token = next(self.generator)
        if type(token) in allowed_tokens:
            return type(token), token
        else:
            raise ParserError(
                "Expected %s got ['%s']=[%s]"
                % ([x.identifier for x in allowed_tokens], token.identifier, token),
                self.line,
                self.filename,
                self.linenum,
            )

    def get_next_check_no_white(self, allowed_tokens: list[type]) -> tuple[type, Token]:
        """
        Get the next acceptable token and strip white space tokens.

        :param allowed_tokens: list of allowed tokens
        :returns: the next acceptable token and its type
        :raises: :py:class:`ParserError` if token is not acceptable
        """
        token = next(self.generator)
        while isinstance(token, LWhite):
            token = next(self.generator)
        if type(token) in allowed_tokens:
            return type(token), token
        else:
            raise ParserError(
                "Expected %s got ['%s']"
                % ([x.identifier for x in allowed_tokens], token.identifier),
                self.line,
                self.filename,
                self.linenum,
            )

    def check_token(
        self, token: Token, allowed_tokens: list[type]
    ) -> tuple[type, Token]:
        """
        Check that a token is acceptable (among the allowed ones).

        :param token: token to check
        :param allowed_tokens: list of allowed tokens
        :returns: the acceptable token and its type
        :raises: :py:class:`ParserError` if token is not acceptable
        """
        if type(token) in allowed_tokens:
            return type(token), token
        else:
            raise ParserError(
                "Expected %s got ['%s']"
                % ([x.identifier for x in allowed_tokens], token.identifier),
                self.line,
                self.filename,
                self.linenum,
            )


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
        self.node = self._parse(Lexer(FileReader(cfgfile)), self.node)
        self.filename = cfgfile

    def parse_string(self, cfgstr: str) -> None:
        """
        Parse a string.

        :param cfgstr: configuration string to parse
        """
        self.node.filename = StrReader("").filename
        self.node = self._parse(Lexer(StrReader(cfgstr)), self.node)

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
    def parse_filter(lexer: Lexer, tokens: list[Token]) -> list[list[Label | Token]]:
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
        typet, token = lexer.check_token(
            next(tokens), [LIdentifier, LLRBracket, LEndL, LWhite]
        )
        and_filter = []
        con_filter = []
        dots = 1

        def next_nw(gener):
            token = next(gener)
            while isinstance(token, LWhite):
                token = next(gener)
            return token

        while typet not in [LEndL]:
            if typet in [LIdentifier, LLRBracket]:  # join    identifier
                if typet == LLRBracket:  # (xxx=ttt)
                    _, ident = lexer.check_token(
                        next_nw(tokens), [LIdentifier]
                    )  # (iden
                    typet, _ = lexer.check_token(
                        next_nw(tokens), [LSet, LRRBracket]
                    )  # =
                    if typet == LRRBracket:  # (xxx)
                        token = Label(str(ident))
                    elif typet == LSet:  # (xxx = yyyy)
                        _, value = lexer.check_token(
                            next_nw(tokens), [LIdentifier, LString]
                        )
                        lexer.check_token(next_nw(tokens), [LRRBracket])
                        token = Label(str(ident), str(value))
                else:
                    token = Label(token)
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
                    or_filters.append([[Label(token)]])
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
                typet, token = lexer.check_token(
                    token, [LIdentifier, LComa, LDot, LLRBracket, LEndL]
                )
                continue
            typet, token = lexer.check_token(
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

    def _parse(self, lexer, node=None, prev_indent=-1):
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

        varianst_allowed_in = [LLBracket, LColon, LIdentifier, LEndL]
        indent_allowed = [LIndent, LEndBlock]

        allowed = block_allowed
        var_indent = 0
        var_name = ""
        # meta contains variants meta-data
        meta = {}
        # pre_dict contains block of operation without collision with
        # others block or operation. Increase speed almost twice.
        pre_dict = {}
        lexer.set_fast()

        # Suffix should be applied as the last operator in the dictionary
        # Reasons:
        #     1. Escape multiply suffix operators
        #     2. Affect all elements in current block
        suffix = None

        def cmd_tokens(tokens1, tokens2):
            for x, y in list(zip(tokens1, tokens2)):
                if x != y:
                    return False
            else:
                return True

        def apply_predict(lexer, node, pre_dict):
            predict = LApplyPreDict().set_operands(None, pre_dict)
            node.content += [(lexer.filename, lexer.linenum, predict)]
            return {}

        try:
            while True:
                lexer.set_prev_indent(prev_indent)
                typet, token = lexer.get_next_check(indent_allowed)
                if typet == LEndBlock:
                    if pre_dict:
                        # flush pre_dict to node content.
                        pre_dict = apply_predict(lexer, node, pre_dict)
                    if suffix:
                        # Node has suffix, apply it to all elements
                        node.content.append(suffix)
                    return node

                indent = token.length
                typet, token = lexer.get_next_check(allowed)

                if typet == LIdentifier:
                    # Parse:
                    #    identifier .....
                    identifier = lexer.get_until_no_white(identifier_allowed)
                    if isinstance(identifier[-1], LOperators):  # operand = <=
                        # Parse:
                        #    identifier = xxx
                        #    identifier <= xxx
                        #    identifier ?= xxx
                        #    etc..
                        op = identifier[-1]
                        if len(identifier) == 1:
                            identifier = token
                        else:
                            identifier = [token] + identifier[:-1]
                            identifier = "".join([str(x) for x in identifier])
                        _, value = lexer.get_next_check([LString])
                        if value and (
                            value[0] == value[-1] == '"' or value[0] == value[-1] == "'"
                        ):
                            value = value[1:-1]

                        op.set_operands(identifier, value)
                        d_nin_val = "$" not in value
                        if isinstance(op, LSet) and d_nin_val:  # Optimization
                            op.apply_to_dict(pre_dict)
                        else:
                            if pre_dict:
                                # flush pre_dict to node content.
                                # If block already contain xxx = yyyy
                                # then operation xxx +=, <=, .... are safe.
                                if op.name in pre_dict and d_nin_val:
                                    op.apply_to_dict(pre_dict)
                                    lexer.get_next_check([LEndL])
                                    continue
                                else:
                                    pre_dict = apply_predict(lexer, node, pre_dict)

                            node.content += [(lexer.filename, lexer.linenum, op)]
                        lexer.get_next_check([LEndL])

                    elif isinstance(identifier[-1], LColon):  # condition:
                        # Parse:
                        #    xxx.yyy.(aaa=bbb):
                        identifier = [token] + identifier[:-1]
                        cfilter = Parser.parse_filter(lexer, identifier + [LEndL()])
                        next_line = lexer.rest_line_as_string_token()
                        if next_line != "":
                            lexer.reader.set_next_line(
                                next_line, indent + 1, lexer.linenum
                            )
                        cond = Condition(cfilter, lexer.line)
                        self._parse(lexer, cond, prev_indent=indent)

                        pre_dict = apply_predict(lexer, node, pre_dict)
                        node.content += [(lexer.filename, lexer.linenum, cond)]
                    else:
                        raise ParserError(
                            'Syntax ERROR expected ":" or' " operand",
                            lexer.line,
                            lexer.filename,
                            lexer.linenum,
                        )

                elif typet == LVariant:
                    # Parse
                    #  - var1: depend1, depend2
                    #      block1
                    #  - var2:
                    #      block2
                    if pre_dict:
                        pre_dict = apply_predict(lexer, node, pre_dict)
                    already_default = False
                    is_default = False
                    meta_with_default = False
                    if "default" in meta:
                        meta_with_default = True
                    meta_in_expand_defautls = False
                    if var_name not in self.expand_defaults:
                        meta_in_expand_defautls = True
                    node4 = Node()
                    while True:
                        lexer.set_prev_indent(var_indent)
                        # Get token from lexer and check syntax.
                        typet, token = lexer.get_next_check_no_white(
                            [LIdentifier, LDefault, LIndent, LEndBlock]
                        )
                        if typet == LEndBlock:
                            break

                        if typet == LIndent:
                            lexer.get_next_check_no_white([LVariant])
                            typet, token = lexer.get_next_check_no_white(
                                [LIdentifier, LDefault]
                            )

                        if typet == LDefault:  # @
                            is_default = True
                            name = lexer.get_until_check([LIdentifier, LDot], [LColon])
                        else:  # identificator
                            is_default = False
                            name = [token] + lexer.get_until_check(
                                [LIdentifier, LDot], [LColon]
                            )

                        if len(name) == 2:
                            name = [name[0]]
                            raw_name = name
                        else:
                            raw_name = [x for x in name[:-1]]
                            name = [x for x in name[:-1] if isinstance(x, LIdentifier)]

                        token = next(lexer.generator)
                        while isinstance(token, LWhite):
                            token = next(lexer.generator)
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

                        if var_name:
                            op = LSet().set_operands(
                                var_name, ".".join([str(n) for n in name])
                            )
                            node2.content += [(lexer.filename, lexer.linenum, op)]

                        node3 = self._parse(lexer, node2, prev_indent=indent)

                        if var_name:
                            node3.var_name = var_name
                            node3.name = [Label(var_name, str(n)) for n in name]
                        else:
                            node3.name = [Label(str(n)) for n in name]

                        # Update mapping name to file

                        node3.dep = deps

                        if meta_with_default:
                            for wd in meta["default"]:
                                if cmd_tokens(wd, raw_name):
                                    is_default = True
                                    meta["default"].remove(wd)

                        if (
                            is_default
                            and not already_default
                            and meta_in_expand_defautls
                        ):
                            node3.default = True
                            already_default = True

                        node3.append_to_shortname = not is_default

                        op = LUpdateFileMap()
                        op.set_operands(
                            lexer.filename, ".".join(str(x) for x in node3.name)
                        )
                        node3.content += [(lexer.filename, lexer.linenum, op)]

                        op = LUpdateFileMap()
                        op.set_operands(
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
                    allowed = block_allowed
                    node = node4

                elif typet == LVariants:  # _name_ [meta1=xxx] [yyy] [xxx]
                    # Parse
                    #    variants _name_ [meta1] [meta2]:
                    if type(node) in [Condition, NegativeCondition]:
                        raise ParserError(
                            "'variants' is not allowed inside a " "conditional block",
                            lexer.line,
                            lexer.reader.filename,
                            lexer.linenum,
                        )

                    lexer.set_strict()
                    tokens = lexer.get_until_no_white(
                        [LLBracket, LColon, LIdentifier, LEndL]
                    )
                    vtypet = type(tokens[-1])
                    var_name = ""
                    meta.clear()
                    # [meta1=xxx] [yyy] [xxx]
                    while vtypet not in [LColon, LEndL]:
                        if vtypet == LIdentifier:
                            if var_name != "":
                                raise ParserError(
                                    "Syntax ERROR expected" ' "[" or ":"',
                                    lexer.line,
                                    lexer.filename,
                                    lexer.linenum,
                                )
                            var_name = tokens[0]
                        elif vtypet == LLBracket:  # [
                            _, ident = lexer.get_next_check_no_white([LIdentifier])
                            typet, _ = lexer.get_next_check_no_white([LSet, LRBracket])
                            if typet == LRBracket:  # [xxx]
                                if ident not in meta:
                                    meta[ident] = []
                                meta[ident].append(True)
                            elif typet == LSet:  # [xxx = yyyy]
                                tokens = lexer.get_until_no_white([LRBracket, LEndL])
                                if isinstance(tokens[-1], LRBracket):
                                    if ident not in meta:
                                        meta[ident] = []
                                    meta[ident].append(tokens[:-1])
                                else:
                                    raise ParserError(
                                        "Syntax ERROR" ' expected "]"',
                                        lexer.line,
                                        lexer.filename,
                                        lexer.linenum,
                                    )

                        tokens = lexer.get_next_check_no_white(varianst_allowed_in)
                        vtypet = type(tokens[-1])

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
                    lexer.get_next_check_no_white([LEndL])
                    allowed = variants_allowed
                    var_indent = indent

                elif typet in [LNo, LOnly]:
                    # Parse:
                    #    only/no (filter=text)..aaa.bbb, xxxx
                    lfilter = Parser.parse_filter(lexer, lexer.rest_line())

                    pre_dict = apply_predict(lexer, node, pre_dict)
                    if typet == LOnly:
                        node.content += [
                            (
                                lexer.filename,
                                lexer.linenum,
                                OnlyFilter(lfilter, lexer.line),
                            )
                        ]
                    else:  # LNo
                        node.content += [
                            (
                                lexer.filename,
                                lexer.linenum,
                                NoFilter(lfilter, lexer.line),
                            )
                        ]

                elif typet == LJoin:
                    # Parse:
                    #    join (filter=text)..aaa.bbb, xxxx
                    # syntax is the same as for No/Only filters
                    lfilter = Parser.parse_filter(lexer, lexer.rest_line())

                    pre_dict = apply_predict(lexer, node, pre_dict)

                    node.content += [
                        (lexer.filename, lexer.linenum, JoinFilter(lfilter, lexer.line))
                    ]

                elif typet == LSuffix:
                    # Parse:
                    #    suffix SUFFIX
                    if pre_dict:
                        pre_dict = apply_predict(lexer, node, pre_dict)
                    token_type, token_val = lexer.get_next_check([LIdentifier])
                    lexer.get_next_check([LEndL])
                    suffix_operator = Suffix().set_operands(None, token_val)
                    # Suffix will be applied as all other elements in current node are processed:
                    suffix = (lexer.filename, lexer.linenum, suffix_operator)

                elif typet == LInclude:
                    # Parse:
                    #    include relative file patch to working directory.
                    path = lexer.rest_line_as_string_token()
                    filename = os.path.expanduser(path)
                    if isinstance(lexer.reader, FileReader) and not os.path.isabs(
                        filename
                    ):
                        filename = os.path.join(
                            os.path.dirname(lexer.filename), filename
                        )
                    if not os.path.isfile(filename):
                        raise MissingIncludeError(
                            lexer.line, lexer.filename, lexer.linenum
                        )
                    pre_dict = apply_predict(lexer, node, pre_dict)
                    lch = Lexer(FileReader(filename))
                    node = self._parse(lch, node, -1)
                    lexer.set_prev_indent(prev_indent)

                elif typet == LDel:
                    # Parse:
                    #    del operand
                    _, to_del = lexer.get_next_check_no_white([LString, LIdentifier])
                    print(0, to_del)
                    #_, to_del = lexer.get_next_check([LIdentifier])
                    #print(1, to_del)
                    #_, to_del = lexer.get_next_check_nw([LString])
                    #print(2, to_del)
                    #if value and (value[0] == value[-1] == '"' or
                    #              value[0] == value[-1] == "'"):
                    #    value = value[1:-1]
                    lexer.get_next_check_no_white([LEndL])
                    token.set_operands(to_del, None)

                    pre_dict = apply_predict(lexer, node, pre_dict)
                    node.content += [(lexer.filename, lexer.linenum, token)]

                elif typet == LNotCond:
                    # Parse:
                    #    !xxx.yyy.(aaa=bbb): vvv
                    lfilter = Parser.parse_filter(
                        lexer, lexer.get_until_no_white([LColon, LEndL])[:-1]
                    )
                    next_line = lexer.rest_line_as_string_token()
                    if next_line != "":
                        lexer.reader.set_next_line(next_line, indent + 1, lexer.linenum)
                    cond = NegativeCondition(lfilter, lexer.line)
                    self._parse(lexer, cond, prev_indent=indent)
                    lexer.set_prev_indent(prev_indent)

                    pre_dict = apply_predict(lexer, node, pre_dict)
                    node.content += [(lexer.filename, lexer.linenum, cond)]
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
        content: list[tuple[str, int, Token]] = None,
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
        content: list[tuple[str, int, Token]] = None,
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
                if isinstance(obj, LOperators):
                    new_content.append(t)
                    continue
                # obj is an OnlyFilter/NoFilter/Condition/NegativeCondition
                if obj.requires_action(ctx, ctx_set, labels):
                    # This filter requires action now
                    if type(obj) is OnlyFilter or type(obj) is NoFilter:
                        if obj not in blocked_filters:
                            self._debug(
                                "    filter did not pass: %r (%s:%s)",
                                obj.line,
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
                            obj.line,
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
                elif obj.is_irrelevant(ctx, ctx_set, labels):
                    # This filter is no longer relevant and can be removed
                    continue
                else:
                    # Keep the filter and check it again later
                    new_content.append(t)
            return True

        def might_pass(
            failed_ctx, failed_ctx_set, failed_external_filters, failed_internal_filters
        ):
            all_content = content + node.content
            for t in failed_external_filters + failed_internal_filters:
                if t not in all_content:
                    return True
            for t in failed_external_filters:
                _, _, external_filter = t
                if not external_filter.might_pass(
                    failed_ctx, failed_ctx_set, ctx, ctx_set, labels
                ):
                    return False
            for t in failed_internal_filters:
                if t not in node.content:
                    return True

            for t in failed_internal_filters:
                _, _, internal_filter = t
                if not internal_filter.might_pass(
                    failed_ctx, failed_ctx_set, ctx, ctx_set, labels
                ):
                    return False
            return True

        def add_failed_case():
            node.failed_cases.appendleft(
                (ctx, ctx_set, new_external_filters, new_internal_filters)
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
        ctx_set = set(ctx)
        labels = node.labels
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
        content: list[tuple[str, int, Token]] = None,
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
        content: list[tuple[str, int, Token]] = None,
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
