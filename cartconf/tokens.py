"""
Tokens module.
"""

# TODO: cannot import in a more natural way, see
# https://github.com/PyO3/pyo3/issues/759
# from .cartconf.tokens import Tokens
from .cartconf import tokens


Tokens = tokens.Tokens


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


LSet = Tokens.LSet
LAppend = Tokens.LAppend
LPrepend = Tokens.LPrepend
LLazySet = Tokens.LLazySet
LRegExpSet = Tokens.LRegExpSet
LRegExpAppend = Tokens.LRegExpAppend
LRegExpPrepend = Tokens.LRegExpPrepend
LDel = Tokens.LDel
LApplyPreDict = Tokens.LApplyPreDict
LUpdateFileMap = Tokens.LUpdateFileMap
Suffix = Tokens.Suffix
tokens_oper = {
    "": LSet,
    "~": LLazySet,
    "+": LAppend,
    "<": LPrepend,
    "?": LRegExpSet,
    "?+": LRegExpAppend,
    "?<": LRegExpPrepend,
    "del": LDel,
    "apply_pre_dict": LApplyPreDict,
    "update_file_map": LUpdateFileMap,
    "suffix": Suffix,
}


def tokens_oper_key(token: "Token") -> str:
    """
    Static key used to identify a token operator class.

    :param token: the token to get the key for.
    :returns: the key used in the operator map with the token type as value
    """
    return str(token).replace("=", "").split(" ", 1)[0]
