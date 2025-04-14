"""
Tokens module.
"""

import collections
import os
import re
from typing import Any

# python imports
from .constants import reserved_keys

# rust imports
# TODO: cannot import in a more natural way, see
# https://github.com/PyO3/pyo3/issues/759
# from .cartconf.tokens import Tokens
from .cartconf import tokens


Tokens = tokens.Tokens
match_substitute = re.compile(r"\$\{(.+?)\}")


class Token(object):
    __slots__ = []
    identifier = ""

    def __str__(self) -> str:
        return self.identifier

    def __repr__(self) -> str:
        return "'%s'" % self.identifier

    def __ne__(self, o: "Token") -> bool:
        """
        The comparison is asymmetric due to optimization.
        """
        if o.identifier != self.identifier:
            return True
        return False


LIndent = Tokens.LIndent
LEndL = Tokens.LEndL
LEndBlock = Tokens.LEndBlock
LIdentifier = Tokens.LIdentifier
LWhite = Tokens.LWhite
LString = Tokens.LString
LColon = Tokens.LColon
LVariants = Tokens.LVariants
LDot = Tokens.LDot
LVariant = Tokens.LVariant
LDefault = Tokens.LDefault
LOnly = Tokens.LOnly
LSuffix = Tokens.LSuffix
LJoin = Tokens.LJoin
LNo = Tokens.LNo
LCond = Tokens.LCond
LNotCond = Tokens.LNotCond
LOr = Tokens.LOr
LAnd = Tokens.LAnd
LCoc = Tokens.LCoc
LComa = Tokens.LComa
LLBracket = Tokens.LLBracket
LRBracket = Tokens.LRBracket
LLRBracket = Tokens.LLRBracket
LRRBracket = Tokens.LRRBracket
LRegExpStart = Tokens.LRegExpStart
LRegExpStop = Tokens.LRegExpStop
LInclude = Tokens.LInclude


class LOperators(Token):
    __slots__ = ["name", "value"]
    identifier = ""
    function = None

    def __init__(self, name: str = "", value: str = "") -> None:
        # pylint: disable=W0201
        self.name = str(name)
        # pylint: disable=W0201
        self.value = str(value)


class LSet(LOperators):
    __slots__ = []
    identifier = "="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        """
        :param d: Dictionary for apply value
        """
        if self.name not in reserved_keys:
            d[self.name] = tokens.substitution(self.value, d)


class LAppend(LOperators):
    __slots__ = []
    identifier = "+="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        if self.name not in reserved_keys:
            d[self.name] = d.get(self.name, "") + tokens.substitution(self.value, d)


class LPrepend(LOperators):
    __slots__ = []
    identifier = "<="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        if self.name not in reserved_keys:
            d[self.name] = tokens.substitution(self.value, d) + d.get(self.name, "")


class LLazySet(LOperators):
    __slots__ = []
    identifier = "~="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        if self.name not in reserved_keys and self.name not in d:
            d[self.name] = tokens.substitution(self.value, d)


class LRegExpSet(LOperators):
    __slots__ = []
    identifier = "?="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        value = tokens.substitution(self.value, d)
        for key in d:
            keystr = "".join(key) if isinstance(key, tuple) else key
            if key not in reserved_keys and exp.match(keystr):
                d[key] = value


class LRegExpAppend(LOperators):
    __slots__ = []
    identifier = "?+="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        value = tokens.substitution(self.value, d)
        for key in d:
            keystr = "".join(key) if isinstance(key, tuple) else key
            if key not in reserved_keys and exp.match(keystr):
                d[key] += value


class LRegExpPrepend(LOperators):
    __slots__ = []
    identifier = "?<="

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        exp = re.compile("%s$" % self.name)
        value = tokens.substitution(self.value, d)
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

    def __init__(self, name: str, value: dict[str, Any]) -> None:
        self.name = name  # pylint: disable=W0201,E0237
        self.value = value  # pylint: disable=W0201,E0237

    def apply_to_dict(self, d: dict[str, Any]) -> None:
        d.update(self.value)

    def __str__(self) -> str:
        return "Apply_pre_dict: %s" % self.value

    def __repr__(self) -> str:
        return "Apply_pre_dict: %s" % self.value


class LUpdateFileMap(LOperators):
    __slots__ = ["shortname", "dest"]
    identifier = "update_file_map"

    def __init__(self, filename: str, name: str, dest: str = "_name_map_file") -> None:
        # pylint: disable=W0201
        self.name = name
        # pylint: disable=W0201
        if filename == "<string>":
            self.shortname = filename
        else:
            self.shortname = os.path.basename(filename)

        self.dest = dest

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


tokens_map = {
    "-": LVariant,
    ".": LDot,
    ":": LColon,
    "@": LDefault,
    ",": LComa,
    "[": LLBracket,
    "]": LRBracket,
    "(": LLRBracket,
    ")": LRRBracket,
    "!": LNotCond,
}


tokens_oper = {
    "": LSet,
    "~": LLazySet,
    "+": LAppend,
    "<": LPrepend,
    "?": LRegExpSet,
    "?+": LRegExpAppend,
    "?<": LRegExpPrepend,
}
