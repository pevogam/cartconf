import unittest
import os
import sys

# simple magic for using scripts within a source tree
basedir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if os.path.isdir(os.path.join(basedir, 'cartconf')):
    sys.path.append(basedir)

from cartconf.tokens import Token, LIndent, LEndL, LEndBlock, LIdentifier, LWhite, LString, LColon, LVariants, LDot, LVariant, LDefault, LOnly, LSuffix, LJoin, LNo, LCond, LNotCond, LOr, LAnd, LCoc, LComa, LLBracket, LRBracket, LLRBracket, LRRBracket, LRegExpStart, LRegExpStop, LInclude, LSet, LAppend, LPrepend, LLazySet, LRegExpSet, LRegExpAppend, LRegExpPrepend, LDel, LApplyPreDict, LUpdateFileMap, Suffix


class TestTokens(unittest.TestCase):

    def test_token(self):
        t = Token()
        self.assertEqual(str(t), "")
        self.assertEqual(repr(t), "''")
        self.assertFalse(t != Token())
        # TODO: the comparison is asymmetric for performance reasons
        self.assertFalse(t == Token())

    def test_lindent(self):
        t = LIndent(4)
        self.assertEqual(str(t), "indent 4")
        self.assertEqual(repr(t), "indent 4")

    def test_lendl(self):
        t = LEndL()
        self.assertEqual(str(t), "endl")
        self.assertEqual(repr(t), "'endl'")

    def test_lendblock(self):
        t = LEndBlock(4)
        self.assertEqual(str(t), "indent 4")
        self.assertEqual(repr(t), "indent 4")

    def test_lidentifier(self):
        t = LIdentifier("identifier")
        self.assertEqual(str(t), "identifier")
        self.assertEqual(repr(t), "'identifier'")

    def test_lwhite(self):
        t = LWhite(" ")
        self.assertEqual(str(t), " ")
        self.assertEqual(repr(t), "' '")

    def test_lstring(self):
        t = LString("string")
        self.assertEqual(str(t), "string")
        self.assertEqual(repr(t), "'string'")

    def test_lcolon(self):
        t = LColon()
        self.assertEqual(str(t), ":")
        self.assertEqual(repr(t), "':'")

    def test_lvariants(self):
        t = LVariants()
        self.assertEqual(str(t), "variants")
        self.assertEqual(repr(t), "'variants'")

    def test_ldot(self):
        t = LDot()
        self.assertEqual(str(t), ".")
        self.assertEqual(repr(t), "'.'")

    def test_lvariant(self):
        t = LVariant()
        self.assertEqual(str(t), "-")
        self.assertEqual(repr(t), "'-'")

    def test_ldefault(self):
        t = LDefault()
        self.assertEqual(str(t), "@")
        self.assertEqual(repr(t), "'@'")

    def test_lonly(self):
        t = LOnly()
        self.assertEqual(str(t), "only")
        self.assertEqual(repr(t), "'only'")

    def test_lsuffix(self):
        t = LSuffix()
        self.assertEqual(str(t), "suffix")
        self.assertEqual(repr(t), "'suffix'")

    def test_ljoin(self):
        t = LJoin()
        self.assertEqual(str(t), "join")
        self.assertEqual(repr(t), "'join'")

    def test_lno(self):
        t = LNo()
        self.assertEqual(str(t), "no")
        self.assertEqual(repr(t), "'no'")

    def test_lcond(self):
        t = LCond()
        self.assertEqual(str(t), "")
        self.assertEqual(repr(t), "''")

    def test_lnotcond(self):
        t = LNotCond()
        self.assertEqual(str(t), "!")
        self.assertEqual(repr(t), "'!'")

    def test_lor(self):
        t = LOr()
        self.assertEqual(str(t), ",")
        self.assertEqual(repr(t), "','")

    def test_land(self):
        t = LAnd()
        self.assertEqual(str(t), "..")
        self.assertEqual(repr(t), "'..'")

    def test_lcoc(self):
        t = LCoc()
        self.assertEqual(str(t), ".")
        self.assertEqual(repr(t), "'.'")

    def test_lcoma(self):
        t = LComa()
        self.assertEqual(str(t), ",")
        self.assertEqual(repr(t), "','")

    def test_llbracket(self):
        t = LLBracket()
        self.assertEqual(str(t), "[")
        self.assertEqual(repr(t), "'['")

    def test_lrbracket(self):
        t = LRBracket()
        self.assertEqual(str(t), "]")
        self.assertEqual(repr(t), "']'")

    def test_llrbracket(self):
        t = LLRBracket()
        self.assertEqual(str(t), "(")
        self.assertEqual(repr(t), "'('")

    def test_lrrbracket(self):
        t = LRRBracket()
        self.assertEqual(str(t), ")")
        self.assertEqual(repr(t), "')'")

    def test_lregexpstart(self):
        t = LRegExpStart()
        self.assertEqual(str(t), "${")
        self.assertEqual(repr(t), "'${'")

    def test_lregexpstop(self):
        t = LRegExpStop()
        self.assertEqual(str(t), "}")
        self.assertEqual(repr(t), "'}'")

    def test_linclude(self):
        t = LInclude()
        self.assertEqual(str(t), "include")
        self.assertEqual(repr(t), "'include'")

    def test_lset(self):
        t = LSet()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "=")
        self.assertEqual(repr(t), "'='")

    def test_lappend(self):
        t = LAppend()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "+=")
        self.assertEqual(repr(t), "'+='")

    def test_lprepend(self):
        t = LPrepend()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "<=")
        self.assertEqual(repr(t), "'<='")

    def test_llazyset(self):
        t = LLazySet()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "~=")
        self.assertEqual(repr(t), "'~='")

    def test_lregexp_set(self):
        t = LRegExpSet()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "?=")
        self.assertEqual(repr(t), "'?='")

    def test_lregexp_append(self):
        t = LRegExpAppend()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "?+=")
        self.assertEqual(repr(t), "'?+='")

    def test_lregexp_prepend(self):
        t = LRegExpPrepend()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "?<=")
        self.assertEqual(repr(t), "'?<='")

    def test_ldel(self):
        t = LDel()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "del")
        self.assertEqual(repr(t), "'del'")

    def test_lapply_pred_dict(self):
        t = LApplyPreDict()
        t.set_operands("name", {"key": "value"})
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, {"key": "value"})
        self.assertEqual(str(t), "Apply_pre_dict: {'key': 'value'}")
        self.assertEqual(repr(t), "Apply_pre_dict: {'key': 'value'}")

    def test_lupdate_file_map(self):
        t = LUpdateFileMap()
        t.set_operands("filename", "name")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.shortname, "filename")
        self.assertEqual(str(t), "update_file_map")
        self.assertEqual(repr(t), "'update_file_map'")

    def test_suffix(self):
        t = Suffix()
        t.set_operands("name", "value")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")
        self.assertEqual(str(t), "Suffix: value")
        self.assertEqual(repr(t), "Suffix value")

if __name__ == '__main__':
    unittest.main()
