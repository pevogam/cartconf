"""
Module for readers, lexers, and parsers as well as their components.
"""

import os
import collections
import logging
import re

from .exceptions import *
from .utils import drop_suffixes
from .filters import *
from .tokens import *


LOG = logging.getLogger('avocado.' + __name__)


tokens_oper_re = [r"\=", r"\+\=", r"\<\=", r"\~\=", r"\?\=", r"\?\+\=", r"\?\<\="]
_ops_exp = re.compile(r"|".join(tokens_oper_re))


class Label(object):
    __slots__ = ["name", "var_name", "long_name", "hash_val", "hash_var"]

    def __init__(self, name, next_name=None):
        if next_name is None:
            self.name = name
            self.var_name = None
        else:
            self.name = next_name
            self.var_name = name

        if self.var_name is None:
            self.long_name = "%s" % (self.name)
        else:
            self.long_name = "(%s=%s)" % (self.var_name, self.name)

        self.hash_val = self.hash_name()
        self.hash_var = None
        if self.var_name:
            self.hash_var = self.hash_variant()

    def __str__(self):
        return self.long_name

    def __repr__(self):
        return self.long_name

    def __eq__(self, o):
        """
        The comparison is asymmetric due to optimization.
        """
        if o.var_name:
            if self.long_name == o.long_name:
                return True
        else:
            if self.name == o.name:
                return True
        return False

    def __ne__(self, o):
        """
        The comparison is asymmetric due to optimization.
        """
        if o.var_name:
            if self.long_name != o.long_name:
                return True
        else:
            if self.name != o.name:
                return True
        return False

    def __hash__(self):
        return self.hash_val

    def hash_name(self):
        return sum([i + 1 * ord(x) for i, x in enumerate(self.name)])

    def hash_variant(self):
        return sum([i + 1 * ord(x) for i, x in enumerate(str(self))])


class Node(object):
    __slots__ = ["var_name", "name", "filename", "dep", "content", "children",
                 "labels", "append_to_shortname", "failed_cases", "default",
                 "q_dict"]

    def __init__(self):
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

    def dump(self, indent, recurse=False):
        print("%s%s" % (" " * indent, self.name))
        print("%s%s" % (" " * indent, self.var_name))
        print("%s%s" % (" " * indent, self))
        print("%s%s" % (" " * indent, self.content))
        print("%s%s" % (" " * indent, self.failed_cases))
        if recurse:
            for child in self.children:
                child.dump(indent + 3, recurse)


class StrReader(object):

    """
    Preprocess an input string for easy reading.
    """

    def __init__(self, s):
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
            if (not stripped_line or
                    stripped_line.startswith("#") or
                    stripped_line.startswith("//")):
                continue
            self._lines.append((stripped_line, indent, linenum + 1))

    def get_next_line(self, prev_indent):
        """
        Get the next line in the current block.

        :param prev_indent: The indentation level of the previous block.
        :return: (line, indent, linenum), where indent is the line's
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

    def set_next_line(self, line, indent, linenum):
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

    def __init__(self, filename):
        """
        Initialize the reader.

        :parse filename: The name of the input file.
        """
        with open(filename) as f:
            StrReader.__init__(self, f.read())
        self.filename = filename


spec_iden = "_-"
spec_oper = "+<?~"


class Lexer(object):

    def __init__(self, reader):
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

    def set_prev_indent(self, prev_indent):
        self.prev_indent = prev_indent

    def set_fast(self):
        self.fast = True

    def set_strict(self):
        self.fast = False

    def match(self, line, pos):
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
            m = _ops_exp.search(line[pos:])

        oper = ""
        token = None

        if self.rest_as_string:
            self.rest_as_string = False
            yield LString(line[pos:].lstrip())
        elif self.fast and m and (cind < 0 or cind > m.end()):
            chars = ""
            yield LIdentifier(line[:m.start()].rstrip())
            yield tokens_oper[m.group()[:-1]]()
            yield LString(line[m.end():].lstrip())
        else:
            li = enumerate(line[pos:], pos)
            for pos, char in li:
                if char.isalnum() or char in spec_iden:    # alfanum+_-
                    chars += char
                elif char in spec_oper:     # <+?=~
                    if chars:
                        yield LIdentifier(chars)
                        oper = ""
                    chars = ""
                    oper += char
                else:
                    if chars:
                        yield LIdentifier(chars)
                        chars = ""
                    if char.isspace():   # Whitespace
                        for pos, char in li:
                            if not char.isspace():
                                if not self.ignore_white:
                                    yield LWhite()
                                break
                    if char.isalnum() or char in spec_iden:
                        chars += char
                    elif char == "=":
                        if oper in tokens_oper:
                            yield tokens_oper[oper]()
                        else:
                            raise LexerError("Unexpected character %s on"
                                             " pos %s" % (char, pos),
                                             self.line, self.filename,
                                             self.linenum)
                        oper = ""
                    elif char in tokens_map:
                        token = tokens_map[char]()
                    elif char == "\"":
                        chars = ""
                        pos, char = next(li)
                        while char != "\"":
                            chars += char
                            pos, char = next(li)
                        yield LString(chars)
                    elif char == "#":
                        break
                    elif char in spec_oper:
                        oper += char
                    else:
                        raise LexerError("Unexpected character %s on"
                                         " pos %s. Special chars are allowed"
                                         " only in variable assignation"
                                         " statement" % (char, pos), line,
                                         self.filename, self.linenum)
                    if token is not None:
                        yield token
                        token = None
                    if self.rest_as_string:
                        self.rest_as_string = False
                        yield LString(line[pos + 1:].lstrip())
                        break
        if chars:
            yield LIdentifier(chars)
            chars = ""
        yield LEndL()

    def get_lexer(self):
        cr = self.reader
        indent = 0
        while True:
            (self.line, indent,
             self.linenum) = cr.get_next_line(self.prev_indent)

            if not self.line:
                yield LEndBlock(indent)
                continue

            yield LIndent(indent)
            for token in self.match(self.line, 0):
                yield token

    def get_until_gen(self, end_tokens=None):
        if end_tokens is None:
            end_tokens = [LEndL]
        token = next(self.generator)
        while type(token) not in end_tokens:
            yield token
            token = next(self.generator)
        yield token

    def get_until(self, end_tokens=None):
        if end_tokens is None:
            end_tokens = [LEndL]
        return [x for x in self.get_until_gen(end_tokens)]

    def flush_until(self, end_tokens=None):
        if end_tokens is None:
            end_tokens = [LEndL]
        for _ in self.get_until_gen(end_tokens):
            pass

    def get_until_check(self, lType, end_tokens=None):
        """
        Read tokens from iterator until get end_tokens or type of token not
        match ltype

        :param lType: List of allowed tokens
        :param end_tokens: List of tokens for end reading
        :return: List of readed tokens.
        """
        if end_tokens is None:
            end_tokens = [LEndL]
        tokens = []
        lType = lType + end_tokens
        for token in self.get_until_gen(end_tokens):
            if type(token) in lType:
                tokens.append(token)
            else:
                raise ParserError("Expected %s got %s" % (lType, type(token)),
                                  self.line, self.filename, self.linenum)
        return tokens

    def get_until_no_white(self, end_tokens=None):
        """
        Read tokens from iterator until get one of end_tokens and strip LWhite

        :param end_tokens:  List of tokens for end reading
        :return: List of readed tokens.
        """
        if end_tokens is None:
            end_tokens = [LEndL]
        return [x for x in self.get_until_gen(end_tokens) if not isinstance(x, LWhite)]

    def rest_line_gen(self):
        token = next(self.generator)
        while not isinstance(token, LEndL):
            yield token
            token = next(self.generator)

    def rest_line(self):
        return [x for x in self.rest_line_gen()]

    def rest_line_no_white(self):
        return [x for x in self.rest_line_gen() if not isinstance(x, LWhite)]

    def rest_line_as_LString(self):
        self.rest_as_string = True
        lstr = next(self.generator)
        next(self.generator)
        return lstr

    def get_next_check(self, lType):
        token = next(self.generator)
        if type(token) in lType:
            return type(token), token
        else:
            raise ParserError("Expected %s got ['%s']=[%s]" %
                              ([x.identifier for x in lType],
                               token.identifier, token),
                              self.line, self.filename, self.linenum)

    def get_next_check_nw(self, lType):
        token = next(self.generator)
        while isinstance(token, LWhite):
            token = next(self.generator)
        if type(token) in lType:
            return type(token), token
        else:
            raise ParserError("Expected %s got ['%s']" %
                              ([x.identifier for x in lType],
                               token.identifier),
                              self.line, self.filename, self.linenum)

    def check_token(self, token, lType):
        if type(token) in lType:
            return type(token), token
        else:
            raise ParserError("Expected %s got ['%s']" %
                              ([x.identifier for x in lType],
                               token.identifier),
                              self.line, self.filename, self.linenum)


def next_nw(gener):
    token = next(gener)
    while isinstance(token, LWhite):
        token = next(gener)
    return token


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


def parse_filter(lexer, tokens):
    """
    :return: Parsed filter
    """
    or_filters = []
    tokens = iter(tokens + [LEndL()])
    typet, token = lexer.check_token(next(tokens), [LIdentifier, LLRBracket,
                                                    LEndL, LWhite])
    and_filter = []
    con_filter = []
    dots = 1
    while typet not in [LEndL]:
        if typet in [LIdentifier, LLRBracket]:        # join    identifier
            if typet == LLRBracket:    # (xxx=ttt)
                _, ident = lexer.check_token(next_nw(tokens),
                                             [LIdentifier])  # (iden
                typet, _ = lexer.check_token(next_nw(tokens),
                                             [LSet, LRRBracket])  # =
                if typet == LRRBracket:  # (xxx)
                    token = Label(str(ident))
                elif typet == LSet:    # (xxx = yyyy)
                    _, value = lexer.check_token(next_nw(tokens),
                                                 [LIdentifier, LString])
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
                raise ParserError("Syntax Error expected \".\" between"
                                  " Identifier.", lexer.line, lexer.filename,
                                  lexer.linenum)

            dots = 0
        elif typet == LDot:         # xxx.xxxx or xxx..xxxx
            dots += 1
        elif typet in [LComa, LWhite]:
            if dots > 0:
                raise ParserError("Syntax Error expected identifier between"
                                  " \".\" and \",\".", lexer.line,
                                  lexer.filename, lexer.linenum)
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
                raise ParserError("Syntax Error expected \",\" between"
                                  " Identifier.", lexer.line, lexer.filename,
                                  lexer.linenum)
            dots = 1
            token = next(tokens)
            while isinstance(token, LWhite):
                token = next(tokens)
            typet, token = lexer.check_token(token, [LIdentifier,
                                                     LComa, LDot,
                                                     LLRBracket, LEndL])
            continue
        typet, token = lexer.check_token(next(tokens), [LIdentifier, LComa,
                                                        LDot, LLRBracket,
                                                        LEndL, LWhite])
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


num_failed_cases = 5


class Parser(object):
    # pylint: disable=W0102

    def __init__(self, filename=None, defaults=False, expand_defaults=[],
                 debug=False):
        self.node = Node()
        self.debug = debug
        self.defaults = defaults
        self.expand_defaults = [LIdentifier(x) for x in expand_defaults]

        self.filename = filename
        if self.filename:
            self.parse_file(self.filename)

        self.only_filters = []
        self.no_filters = []
        self.assignments = []

        # get_dicts() - is recursive generator, it can invoke itself,
        # as well as it can be called outside to get dic list
        # It is necessary somehow mark top-level generator,
        # to be able process all variables, do suffix stuff, drops dups, etc....
        # It can be safely done only on top top level get_dicts()
        # Parent generator will reset this flag
        self.parent_generator = True

    def _debug(self, s, *args):
        if self.debug:
            LOG.debug(s, *args)

    def _warn(self, s, *args):
        LOG.warn(s, *args)

    def parse_file(self, filename):
        """
        Parse a file.

        :param filename: Path of the configuration file.
        """
        self.node.filename = filename
        self.node = self._parse(Lexer(FileReader(filename)), self.node)
        self.filename = filename

    def parse_string(self, s):
        """
        Parse a string.

        :param s: String to parse.
        """
        self.node.filename = StrReader("").filename
        self.node = self._parse(Lexer(StrReader(s)), self.node)

    def only_filter(self, variant):
        """
        Apply a only filter programatically and keep track of it.

        Equivalent to parse a "only variant" line.

        :param variant: String with the variant name.
        """
        string = "only %s" % variant
        self.only_filters.append(string)
        self.parse_string(string)

    def no_filter(self, variant):
        """
        Apply a no filter programatically and keep track of it.

        Equivalent to parse a "no variant" line.

        :param variant: String with the variant name.
        """
        string = "no %s" % variant
        self.no_filters.append(string)
        self.parse_string(string)

    def assign(self, key, value):
        """
        Apply an assignment programatically and keep track of it.

        Equivalent to parse a "key = value" line.

        :param variant: String with the variant name.
        """
        string = "%s = %s" % (key, value)
        self.assignments.append(string)
        self.parse_string(string)

    def _parse(self, lexer, node=None, prev_indent=-1):
        if not node:
            node = self.node
        block_allowed = [LVariants, LIdentifier, LOnly,
                         LNo, LInclude, LDel, LNotCond, LSuffix, LJoin]

        variants_allowed = [LVariant]

        identifier_allowed = [LSet, LAppend, LPrepend, LLazySet,
                              LRegExpSet, LRegExpAppend,
                              LRegExpPrepend, LColon,
                              LEndL]

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
                        if (len(identifier) == 1):
                            identifier = token
                        else:
                            identifier = [token] + identifier[:-1]
                            identifier = "".join([str(x) for x in identifier])
                        _, value = lexer.get_next_check([LString])
                        if value and (value[0] == value[-1] == '"' or
                                      value[0] == value[-1] == "'"):
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
                                    pre_dict = apply_predict(lexer, node,
                                                             pre_dict)

                            node.content += [(lexer.filename,
                                              lexer.linenum,
                                              op)]
                        lexer.get_next_check([LEndL])

                    elif isinstance(identifier[-1], LColon):  # condition:
                        # Parse:
                        #    xxx.yyy.(aaa=bbb):
                        identifier = [token] + identifier[:-1]
                        cfilter = parse_filter(lexer, identifier + [LEndL()])
                        next_line = lexer.rest_line_as_LString()
                        if next_line != "":
                            lexer.reader.set_next_line(next_line, indent + 1,
                                                       lexer.linenum)
                        cond = Condition(cfilter, lexer.line)
                        self._parse(lexer, cond, prev_indent=indent)

                        pre_dict = apply_predict(lexer, node, pre_dict)
                        node.content += [(lexer.filename, lexer.linenum, cond)]
                    else:
                        raise ParserError("Syntax ERROR expected \":\" or"
                                          " operand", lexer.line,
                                          lexer.filename, lexer.linenum)

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
                        typet, token = lexer.get_next_check_nw([LIdentifier,
                                                                LDefault,
                                                                LIndent,
                                                                LEndBlock])
                        if typet == LEndBlock:
                            break

                        if typet == LIndent:
                            lexer.get_next_check_nw([LVariant])
                            typet, token = lexer.get_next_check_nw(
                                [LIdentifier,
                                 LDefault])

                        if typet == LDefault:  # @
                            is_default = True
                            name = lexer.get_until_check([LIdentifier, LDot],
                                                         [LColon])
                        else:  # identificator
                            is_default = False
                            name = [token] + lexer.get_until_check(
                                [LIdentifier, LDot],
                                [LColon])

                        if len(name) == 2:
                            name = [name[0]]
                            raw_name = name
                        else:
                            raw_name = [x for x in name[:-1]]
                            name = [x for x in name[:-1]
                                    if isinstance(x, LIdentifier)]

                        token = next(lexer.generator)
                        while isinstance(token, LWhite):
                            token = next(lexer.generator)
                        tokens = None
                        if not isinstance(token, LEndL):
                            tokens = [token] + lexer.get_until([LEndL])
                            deps = parse_filter(lexer, tokens)
                        else:
                            deps = []

                        # Prepare data for dict generator.
                        node2 = Node()
                        node2.children = [node]
                        node2.labels = node.labels

                        if var_name:
                            op = LSet().set_operands(var_name,
                                                     ".".join([str(n) for n in name]))
                            node2.content += [(lexer.filename,
                                               lexer.linenum,
                                               op)]

                        node3 = self._parse(lexer, node2, prev_indent=indent)

                        if var_name:
                            node3.var_name = var_name
                            node3.name = [Label(var_name, str(n))
                                          for n in name]
                        else:
                            node3.name = [Label(str(n)) for n in name]

                        # Update mapping name to file

                        node3.dep = deps

                        if meta_with_default:
                            for wd in meta["default"]:
                                if cmd_tokens(wd, raw_name):
                                    is_default = True
                                    meta["default"].remove(wd)

                        if (is_default and not already_default and
                                meta_in_expand_defautls):
                            node3.default = True
                            already_default = True

                        node3.append_to_shortname = not is_default

                        op = LUpdateFileMap()
                        op.set_operands(lexer.filename,
                                        ".".join(str(x)
                                                 for x in node3.name))
                        node3.content += [(lexer.filename,
                                           lexer.linenum,
                                           op)]

                        op = LUpdateFileMap()
                        op.set_operands(lexer.filename,
                                        ".".join(str(x.name)
                                                 for x in node3.name),
                                        "_short_name_map_file")
                        node3.content += [(lexer.filename,
                                           lexer.linenum,
                                           op)]

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
                        raise ParserError("Missing default variant %s" %
                                          (meta["default"]), lexer.line,
                                          lexer.filename, lexer.linenum)
                    allowed = block_allowed
                    node = node4

                elif typet == LVariants:  # _name_ [meta1=xxx] [yyy] [xxx]
                    # Parse
                    #    variants _name_ [meta1] [meta2]:
                    if type(node) in [Condition, NegativeCondition]:
                        raise ParserError("'variants' is not allowed inside a "
                                          "conditional block", lexer.line,
                                          lexer.reader.filename, lexer.linenum)

                    lexer.set_strict()
                    tokens = lexer.get_until_no_white([LLBracket, LColon,
                                                       LIdentifier, LEndL])
                    vtypet = type(tokens[-1])
                    var_name = ""
                    meta.clear()
                    # [meta1=xxx] [yyy] [xxx]
                    while vtypet not in [LColon, LEndL]:
                        if vtypet == LIdentifier:
                            if var_name != "":
                                raise ParserError("Syntax ERROR expected"
                                                  " \"[\" or \":\"",
                                                  lexer.line, lexer.filename,
                                                  lexer.linenum)
                            var_name = tokens[0]
                        elif vtypet == LLBracket:  # [
                            _, ident = lexer.get_next_check_nw([LIdentifier])
                            typet, _ = lexer.get_next_check_nw([LSet,
                                                                LRBracket])
                            if typet == LRBracket:  # [xxx]
                                if ident not in meta:
                                    meta[ident] = []
                                meta[ident].append(True)
                            elif typet == LSet:  # [xxx = yyyy]
                                tokens = lexer.get_until_no_white([LRBracket,
                                                                   LEndL])
                                if isinstance(tokens[-1], LRBracket):
                                    if ident not in meta:
                                        meta[ident] = []
                                    meta[ident].append(tokens[:-1])
                                else:
                                    raise ParserError("Syntax ERROR"
                                                      " expected \"]\"",
                                                      lexer.line,
                                                      lexer.filename,
                                                      lexer.linenum)

                        tokens = lexer.get_next_check_nw(varianst_allowed_in)
                        vtypet = type(tokens[-1])

                    if "default" in meta:
                        for wd in meta["default"]:
                            if not isinstance(wd, list):
                                raise ParserError("Syntax ERROR expected "
                                                  "[default=xxx]",
                                                  lexer.line,
                                                  lexer.filename,
                                                  lexer.linenum)

                    if vtypet == LEndL:
                        raise ParserError("Syntax ERROR expected \":\"",
                                          lexer.line, lexer.filename,
                                          lexer.linenum)
                    lexer.get_next_check_nw([LEndL])
                    allowed = variants_allowed
                    var_indent = indent

                elif typet in [LNo, LOnly]:
                    # Parse:
                    #    only/no (filter=text)..aaa.bbb, xxxx
                    lfilter = parse_filter(lexer, lexer.rest_line())

                    pre_dict = apply_predict(lexer, node, pre_dict)
                    if typet == LOnly:
                        node.content += [(lexer.filename, lexer.linenum,
                                          OnlyFilter(lfilter, lexer.line))]
                    else:  # LNo
                        node.content += [(lexer.filename, lexer.linenum,
                                          NoFilter(lfilter, lexer.line))]

                elif typet == LJoin:
                    # Parse:
                    #    join (filter=text)..aaa.bbb, xxxx
                    # syntax is the same as for No/Only filters
                    lfilter = parse_filter(lexer, lexer.rest_line())

                    pre_dict = apply_predict(lexer, node, pre_dict)

                    node.content += [(lexer.filename, lexer.linenum, JoinFilter(lfilter, lexer.line))]

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
                    path = lexer.rest_line_as_LString()
                    filename = os.path.expanduser(path)
                    if (isinstance(lexer.reader, FileReader) and
                            not os.path.isabs(filename)):
                        filename = os.path.join(
                            os.path.dirname(lexer.filename),
                            filename)
                    if not os.path.isfile(filename):
                        raise MissingIncludeError(lexer.line, lexer.filename,
                                                  lexer.linenum)
                    pre_dict = apply_predict(lexer, node, pre_dict)
                    lch = Lexer(FileReader(filename))
                    node = self._parse(lch, node, -1)
                    lexer.set_prev_indent(prev_indent)

                elif typet == LDel:
                    # Parse:
                    #    del operand
                    _, to_del = lexer.get_next_check_nw([LIdentifier])
                    lexer.get_next_check_nw([LEndL])
                    token.set_operands(to_del, None)

                    pre_dict = apply_predict(lexer, node, pre_dict)
                    node.content += [(lexer.filename, lexer.linenum,
                                      token)]

                elif typet == LNotCond:
                    # Parse:
                    #    !xxx.yyy.(aaa=bbb): vvv
                    lfilter = parse_filter(lexer,
                                           lexer.get_until_no_white(
                                               [LColon, LEndL])[:-1])
                    next_line = lexer.rest_line_as_LString()
                    if next_line != "":
                        lexer.reader.set_next_line(next_line, indent + 1,
                                                   lexer.linenum)
                    cond = NegativeCondition(lfilter, lexer.line)
                    self._parse(lexer, cond, prev_indent=indent)
                    lexer.set_prev_indent(prev_indent)

                    pre_dict = apply_predict(lexer, node, pre_dict)
                    node.content += [(lexer.filename, lexer.linenum, cond)]
                else:
                    raise ParserError("Syntax ERROR expected", lexer.line,
                                      lexer.filename, lexer.linenum)
        except Exception:
            self._debug("%s  %s:  %s" % (lexer.filename, lexer.linenum,
                                         lexer.line))
            raise

    def get_dicts(self, node=None, ctx=[], content=[], shortname=[], dep=[], skipdups=True):
        """
        Process 'join' entry, unpack join filter for node.

        :param ctx: node labels/names
        :param content: previous content in plain
        :returns: dictionary

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
            for d in self.multiply_join(onlys, node, ctx, content, shortname, dep):
                yield drop_suffixes(d, skipdups=skipdups) if parent else d
            node.content = old_content[:]

    def mk_name(self, n1, n2):
        """Make name for test. Case: two dics were merged"""
        common_prefix = n1[:[x[0] == x[1] for x in list(zip(n1, n2))].index(0)]
        cp = ".".join(common_prefix.split('.')[:-1])
        p1 = re.sub(r"^"+cp, "", n1)
        p2 = re.sub(r"^"+cp, "", n2)
        if cp:
            name = cp + p1 + p2
        else:
            name = p1 + "." + p2
        return name

    def multiply_join(self, onlys, node=None, ctx=[], content=[], shortname=[], dep=[]):
        """
        Multiply all joins. Return dictionaries one by one
        Each `join' is the same as `only' filter
        This functions is supposed to be a generator, recursive generator
        """
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
                for d2 in self.multiply_join(remains, node, ctx, content, shortname, dep):

                    d = d1.copy()
                    d.update(d2)
                    d["name"] = self.mk_name(d1["name"], d2["name"])
                    d["shortname"] = self.mk_name(d1["shortname"], d2["shortname"])
                    yield d

    def get_dicts_plain(self, node=None, ctx=[], content=[], shortname=[], dep=[]):
        """
        Generate dictionaries from the code parsed so far.  This should
        be called after parsing something.

        :return: A dict generator.
        """
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
                            self._debug("    filter did not pass: %r (%s:%s)",
                                        obj.line, filename, linenum)
                            failed_filters.append(t)
                            return False
                        else:
                            continue
                    else:
                        self._debug("    conditional block matches:"
                                    " %r (%s:%s)", obj.line, filename, linenum)
                        # Check and unpack the content inside this Condition
                        # object (note: the failed filters should go into
                        # new_internal_filters because we don't expect them to
                        # come from outside this node, even if the Condition
                        # itself was external)
                        if not process_content(obj.content,
                                               new_internal_filters):
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

        def might_pass(failed_ctx,
                       failed_ctx_set,
                       failed_external_filters,
                       failed_internal_filters):
            all_content = content + node.content
            for t in failed_external_filters + failed_internal_filters:
                if t not in all_content:
                    return True
            for t in failed_external_filters:
                _, _, external_filter = t
                if not external_filter.might_pass(failed_ctx,
                                                  failed_ctx_set,
                                                  ctx, ctx_set,
                                                  labels):
                    return False
            for t in failed_internal_filters:
                if t not in node.content:
                    return True

            for t in failed_internal_filters:
                _, _, internal_filter = t
                if not internal_filter.might_pass(failed_ctx,
                                                  failed_ctx_set,
                                                  ctx, ctx_set,
                                                  labels):
                    return False
            return True

        def add_failed_case():
            node.failed_cases.appendleft((ctx, ctx_set,
                                          new_external_filters,
                                          new_internal_filters))
            if len(node.failed_cases) > num_failed_cases:
                node.failed_cases.pop()

        node = node or self.node
        # if self.debug:    #Print dict on which is working now.
        #    node.dump(0)
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
                self._debug("\n*    this subtree has failed before %s\n"
                            "         content: %s\n"
                            "         failcase:%s\n",
                            name, content + node.content, failed_case)
                del node.failed_cases[i]
                node.failed_cases.appendleft(failed_case)
                return

        # Check content and unpack it into new_content
        new_content = []
        new_external_filters = []
        new_internal_filters = []
        if (not process_content(node.content, new_internal_filters) or
                not process_content(content, new_external_filters)):
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
                for d in self.get_dicts(n, ctx, new_content, shortname, dep):
                    count += 1
                    yield d
                if n.default and count:
                    break
        else:
            for n in node.children:
                for d in self.get_dicts(n, ctx, new_content, shortname, dep):
                    count += 1
                    yield d
        # Reached leaf?
        if not node.children:
            self._debug("    reached leaf, returning it")
            d = {"name": name, "dep": dep,
                 "shortname": ".".join([str(sn.name) for sn in shortname])}
            for _, _, op in new_content:
                op.apply_to_dict(d)
            postfix_parse(d)
            yield d


def convert_data_size(size, default_sufix='B'):
    """
    Convert data size from human readable units to an int of arbitrary size.

    :param size: Human readable data size representation (string).
    :param default_sufix: Default sufix used to represent data.
    :return: Int with data size in the appropriate order of magnitude.
    """
    orders = {'B': 1,
              'K': 1024,
              'M': 1024 * 1024,
              'G': 1024 * 1024 * 1024,
              'T': 1024 * 1024 * 1024 * 1024,
              }

    order = re.findall("([BbKkMmGgTt])", size[-1])
    if not order:
        size += default_sufix
        order = [default_sufix]

    return int(float(size[0:-1]) * orders[order[0].upper()])


def compare_string(str1, str2):
    """
    Compare two int string and return -1, 0, 1.
    It can compare two memory value even in sufix

    :param str1: The first string
    :param str2: The second string

    :Return: Return -1, when str1<  str2
                     0, when str1 = str2
                     1, when str1>  str2
    """
    order1 = re.findall("([BbKkMmGgTt])", str1)
    order2 = re.findall("([BbKkMmGgTt])", str2)
    if order1 or order2:
        value1 = convert_data_size(str1, "M")
        value2 = convert_data_size(str2, "M")
    else:
        value1 = int(str1)
        value2 = int(str2)
    if value1 < value2:
        return -1
    elif value1 == value2:
        return 0
    else:
        return 1


def postfix_parse(dic):
    tmp_dict = {}
    for key in dic:
        # Bypass the case that use tuple as key value
        if isinstance(key, tuple):
            continue
        if key.endswith("_max"):
            tmp_key = key.split("_max")[0]
            if (tmp_key not in dic or
                    compare_string(dic[tmp_key], dic[key]) > 0):
                tmp_dict[tmp_key] = dic[key]
        elif key.endswith("_min"):
            tmp_key = key.split("_min")[0]
            if (tmp_key not in dic or
                    compare_string(dic[tmp_key], dic[key]) < 0):
                tmp_dict[tmp_key] = dic[key]
        elif key.endswith("_fixed"):
            tmp_key = key.split("_fixed")[0]
            tmp_dict[tmp_key] = dic[key]
    for key in tmp_dict:
        dic[key] = tmp_dict[key]
