"""
Tokens module.
"""

import collections
import os
import re
from typing import Any

from .exceptions import ParserError
from .constants import reserved_keys
from .utils import drop_suffixes


match_substitute = re.compile(r"\$\{(.+?)\}")


class Token(object):
    __slots__ = []
    identifier = ""

    def __str__(self) -> str:
        return self.identifier

    def __repr__(self) -> str:
        return "'%s'" % self.identifier

    def __ne__(self, o: 'Token') -> bool:
        """
        The comparison is asymmetric due to optimization.
        """
        if o.identifier != self.identifier:
            return True
        return False


class LIndent(Token):
    __slots__ = ["length"]
    identifier = "indent"

    def __init__(self, length: int) -> None:
        self.length = length

    def __str__(self) -> str:
        return "%s %s" % (self.identifier, self.length)

    def __repr__(self) -> str:
        return "%s %s" % (self.identifier, self.length)


class LEndL(Token):
    __slots__ = []
    identifier = "endl"


class LEndBlock(LIndent):
    __slots__ = []
    pass


class LIdentifier(str):
    __slots__ = []
    identifier = "Identifier re([A-Za-z0-9][A-Za-z0-9_-]*)"

    def __str__(self) -> str:
        return super(LIdentifier, self).__str__()

    def __repr__(self) -> str:
        return "'%s'" % self

    def checkChar(self, chars: str) -> 'LIdentifier':
        for t in self:
            if not (t in chars):
                raise ParserError("Wrong char %s in %s" % (t, self))
        return self

    def checkAlpha(self) -> 'LIdentifier':
        """
        Check if string contain only chars
        """
        if not self.isalpha():
            raise ParserError("Some of chars is not alpha in %s" % (self))
        return self

    def checkNumbers(self) -> 'LIdentifier':
        """
        Check if string contain only chars
        """
        if not self.isdigit():
            raise ParserError("Some of chars is not digit in %s" % (self))
        return self

    def checkCharAlpha(self, chars: str) -> 'LIdentifier':
        """
        Check if string contain only chars
        """
        for t in self:
            if not (t in chars or t.isalpha()):
                raise ParserError("Char %s is not alpha or one of special"
                                  "chars [%s] in %s" % (t, chars, self))
        return self

    def checkCharAlphaNum(self, chars: str) -> 'LIdentifier':
        """
        Check if string contain only chars
        """
        for t in self:
            if not (t in chars or t.isalnum()):
                raise ParserError("Char %s is not alphanum or one of special"
                                  "chars [%s] in %s" % (t, chars, self))
        return self

    def checkCharNumeric(self, chars: str) -> 'LIdentifier':
        """
        Check if string contain only chars
        """
        for t in self:
            if not (t in chars or t.isdigit()):
                raise ParserError("Char %s is not digit or one of special"
                                  "chars [%s] in %s" % (t, chars, self))
        return self


class LWhite(LIdentifier):
    __slots__ = []
    identifier = "WhiteSpace re(\\s)"


class LString(LIdentifier):
    __slots__ = []
    identifier = "String re(.+)"


class LColon(Token):
    __slots__ = []
    identifier = ":"


class LVariants(Token):
    __slots__ = []
    identifier = "variants"


class LDot(Token):
    __slots__ = []
    identifier = "."


class LVariant(Token):
    __slots__ = []
    identifier = "-"


class LDefault(Token):
    __slots__ = []
    identifier = "@"


class LOnly(Token):
    __slots__ = []
    identifier = "only"


class LSuffix(Token):
    __slots__ = []
    identifier = "suffix"


class LJoin(Token):
    __slots__ = []
    identifier = "join"


class LNo(Token):
    __slots__ = []
    identifier = "no"


class LCond(Token):
    __slots__ = []
    identifier = ""


class LNotCond(Token):
    __slots__ = []
    identifier = "!"


class LOr(Token):
    __slots__ = []
    identifier = ","


class LAnd(Token):
    __slots__ = []
    identifier = ".."


class LCoc(Token):
    __slots__ = []
    identifier = "."


class LComa(Token):
    __slots__ = []
    identifier = ","


class LLBracket(Token):
    __slots__ = []
    identifier = "["


class LRBracket(Token):
    __slots__ = []
    identifier = "]"


class LLRBracket(Token):
    __slots__ = []
    identifier = "("


class LRRBracket(Token):
    __slots__ = []
    identifier = ")"


class LRegExpStart(Token):
    __slots__ = []
    identifier = "${"


class LRegExpStop(Token):
    __slots__ = []
    identifier = "}"


class LInclude(Token):
    __slots__ = []
    identifier = "include"


class LOperators(Token):
    __slots__ = ["name", "value"]
    identifier = ""
    function = None

    def set_operands(self, name: str, value: str) -> 'LOperators':
        # pylint: disable=W0201
        self.name = str(name)
        # pylint: disable=W0201
        self.value = str(value)
        return self


class LSet(LOperators):
    __slots__ = []
    identifier = "="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        """
        :param d: Dictionary for apply value
        """
        if self.name not in reserved_keys:
            d[self.name] = _substitution(self.value, d)


class LAppend(LOperators):
    __slots__ = []
    identifier = "+="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        if self.name not in reserved_keys:
            d[self.name] = d.get(self.name, "") + _substitution(self.value, d)


class LPrepend(LOperators):
    __slots__ = []
    identifier = "<="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        if self.name not in reserved_keys:
            d[self.name] = _substitution(self.value, d) + d.get(self.name, "")


class LLazySet(LOperators):
    __slots__ = []
    identifier = "~="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        if self.name not in reserved_keys and self.name not in d:
            d[self.name] = _substitution(self.value, d)


class LRegExpSet(LOperators):
    __slots__ = []
    identifier = "?="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        value = _substitution(self.value, d)
        for key in d:
            keystr = "".join(key) if isinstance(key, tuple) else key
            if key not in reserved_keys and exp.match(keystr):
                d[key] = value


class LRegExpAppend(LOperators):
    __slots__ = []
    identifier = "?+="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        value = _substitution(self.value, d)
        for key in d:
            keystr = "".join(key) if isinstance(key, tuple) else key
            if key not in reserved_keys and exp.match(keystr):
                d[key] += value


class LRegExpPrepend(LOperators):
    __slots__ = []
    identifier = "?<="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        value = _substitution(self.value, d)
        for key in d:
            keystr = "".join(key) if isinstance(key, tuple) else key
            if key not in reserved_keys and exp.match(keystr):
                d[key] = value + d[key]


class LDel(LOperators):
    __slots__ = []
    identifier = "del"

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        keys_to_del = collections.deque()
        for key in d:
            keystr = "".join(key) if isinstance(key, tuple) else key
            if key not in reserved_keys and exp.match(keystr):
                keys_to_del.append(key)
        for key in keys_to_del:
            del d[key]


class LApplyPreDict(LOperators):
    __slots__ = []
    identifier = "apply_pre_dict"

    def set_operands(self, name: str, value: dict[str, Any]) -> 'LApplyPreDict':
        self.name = name    # pylint: disable=W0201,E0237
        self.value = value  # pylint: disable=W0201,E0237
        return self

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        d.update(self.value)

    def __str__(self) -> str:
        return "Apply_pre_dict: %s" % self.value

    def __repr__(self) -> str:
        return "Apply_pre_dict: %s" % self.value


class LUpdateFileMap(LOperators):
    __slots__ = ["shortname", "dest"]
    identifier = "update_file_map"

    def set_operands(self, filename: str, name: str, dest: str = "_name_map_file") -> 'LUpdateFileMap':
        # pylint: disable=W0201
        self.name = name
        # pylint: disable=W0201
        if filename == "<string>":
            self.shortname = filename
        else:
            self.shortname = os.path.basename(filename)

        self.dest = dest
        return self

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        dest = self.dest
        if dest not in d:
            d[dest] = {}

        if self.shortname in d[dest]:
            old_name = d[dest][self.shortname]
            d[dest][self.shortname] = "%s.%s" % (self.name, old_name)
        else:
            d[dest][self.shortname] = self.name


class Suffix(LOperators):
    __slots__ = []
    identifier = "apply_suffix"

    def __str__(self) -> str:
        return "Suffix: %s" % (self.value)

    def __repr__(self) -> str:
        return "Suffix %s" % (self.value)

    def __eq__(self, o: Any) -> bool:
        if isinstance(o, self.__class__):
            if self.value == o.value:
                return True
        return False

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        for key in d.copy():
            if key not in reserved_keys:
                # Store key as a tuple: (key, suffix1, suffix2, suffix3,....)
                # This allows us to manipulate later on suffixes
                # Add suffix to the key, remove the old key
                new_key = (key if isinstance(key, tuple) else (key,)) + (self.value,)
                d[new_key] = d.pop(key)


tokens_map = {"-": LVariant,
              ".": LDot,
              ":": LColon,
              "@": LDefault,
              ",": LComa,
              "[": LLBracket,
              "]": LRBracket,
              "(": LLRBracket,
              ")": LRRBracket,
              "!": LNotCond}


tokens_oper = {"": LSet,
               "~": LLazySet,
               "+": LAppend,
               "<": LPrepend,
               "?": LRegExpSet,
               "?+": LRegExpAppend,
               "?<": LRegExpPrepend,
               }


# Helpers for all tokens
def _substitution(value: str, d: dict[str, Any]) -> str:
    """
    Only optimization string Template substitute is quite expensive operation.

    .. todo:: The current substitution is limited to simple cases when it comes
        to using join and suffix operators.

    :param value: String where could be $string for substitution.
    :param d: Dictionary from which should be value substituted to value.

    :return: Substituted string
    """
    if "$" in value:
        start = 0
        st = ""
        d_flat = drop_suffixes(d)
        try:
            match = match_substitute.search(value, start)
            while match:
                val = d_flat[match.group(1)]
                st += value[start:match.start()] + str(val)
                start = match.end()
                match = match_substitute.search(value, start)
        except KeyError:
            pass
        st += value[start:len(value)]
        return st
    else:
        return value
