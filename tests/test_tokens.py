import unittest
import os
import sys

# simple magic for using scripts within a source tree
basedir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if os.path.isdir(os.path.join(basedir, 'cartconf')):
    sys.path.append(basedir)

from cartconf.tokens import LIndent, LEndL, LEndBlock, LIdentifier, LWhite, LString, LColon, LVariants, LDot, LVariant, LDefault, LOnly, LSuffix, LJoin, LNo, LCond, LNotCond, LOr, LAnd, LCoc, LComa, LLBracket, LRBracket, LLRBracket, LRRBracket, LRegExpStart, LRegExpStop, LInclude, LSet, LAppend, LPrepend, LLazySet, LRegExpSet, LRegExpAppend, LRegExpPrepend, LDel, LApplyPreDict, LUpdateFileMap, Suffix


class TestTokens(unittest.TestCase):

    def test_lindent(self):
        t = LIndent(4)
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "indent 4")
        self.assertEqual(repr(t), "'indent 4'")
        self.assertEqual(t.length, 4)

    def test_lendl(self):
        t = LEndL()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "endl")
        self.assertEqual(repr(t), "'endl'")

    def test_lendblock(self):
        t = LEndBlock(4)
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "endb 4")
        self.assertEqual(repr(t), "'endb 4'")
        self.assertEqual(t.length, 4)

    def test_lidentifier(self):
        t = LIdentifier("identifier")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "Identifier re([A-Za-z0-9][A-Za-z0-9_-]*) \"identifier\"")
        self.assertEqual(repr(t), "'Identifier re([A-Za-z0-9][A-Za-z0-9_-]*) \"identifier\"'")
        self.assertEqual(t.string, "identifier")

    def test_lwhite(self):
        t = LWhite(" ")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "WhiteSpace re(\\s) \" \"")
        self.assertEqual(repr(t), "'WhiteSpace re(\\s) \" \"'")
        self.assertEqual(t.string, " ")

    def test_lstring(self):
        t = LString("string")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "String re(.+) \"string\"")
        self.assertEqual(repr(t), "'String re(.+) \"string\"'")
        self.assertEqual(t.string, "string")

    def test_lcolon(self):
        t = LColon()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), ":")
        self.assertEqual(repr(t), "':'")

    def test_lvariants(self):
        t = LVariants()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "variants")
        self.assertEqual(repr(t), "'variants'")

    def test_ldot(self):
        t = LDot()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), ".")
        self.assertEqual(repr(t), "'.'")

    def test_lvariant(self):
        t = LVariant()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "-")
        self.assertEqual(repr(t), "'-'")

    def test_ldefault(self):
        t = LDefault()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "@")
        self.assertEqual(repr(t), "'@'")

    def test_lonly(self):
        t = LOnly()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "only")
        self.assertEqual(repr(t), "'only'")

    def test_lsuffix(self):
        t = LSuffix()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "suffix")
        self.assertEqual(repr(t), "'suffix'")

    def test_ljoin(self):
        t = LJoin()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "join")
        self.assertEqual(repr(t), "'join'")

    def test_lno(self):
        t = LNo()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "no")
        self.assertEqual(repr(t), "'no'")

    def test_lcond(self):
        t = LCond()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "?")
        self.assertEqual(repr(t), "'?'")

    def test_lnotcond(self):
        t = LNotCond()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "!")
        self.assertEqual(repr(t), "'!'")

    def test_lor(self):
        t = LOr()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), ",")
        self.assertEqual(repr(t), "','")

    def test_land(self):
        t = LAnd()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "..")
        self.assertEqual(repr(t), "'..'")

    def test_lcoc(self):
        t = LCoc()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), ".")
        self.assertEqual(repr(t), "'.'")

    def test_lcoma(self):
        t = LComa()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), ",")
        self.assertEqual(repr(t), "','")

    def test_llbracket(self):
        t = LLBracket()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "[")
        self.assertEqual(repr(t), "'['")

    def test_lrbracket(self):
        t = LRBracket()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "]")
        self.assertEqual(repr(t), "']'")

    def test_llrbracket(self):
        t = LLRBracket()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "(")
        self.assertEqual(repr(t), "'('")

    def test_lrrbracket(self):
        t = LRRBracket()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), ")")
        self.assertEqual(repr(t), "')'")

    def test_lregexpstart(self):
        t = LRegExpStart()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "${")
        self.assertEqual(repr(t), "'${'")

    def test_lregexpstop(self):
        t = LRegExpStop()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "}")
        self.assertEqual(repr(t), "'}'")

    def test_linclude(self):
        t = LInclude()
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "include")
        self.assertEqual(repr(t), "'include'")

    def test_lset(self):
        t = LSet("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "=")
        self.assertEqual(repr(t), "'='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_lset_apply(self):
        t = LSet("key", "value")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "value")
        d = {"key": "old_value"}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "value")
        d = {"shortname": "is reserved"}
        LSet("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_lappend(self):
        t = LAppend("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "+=")
        self.assertEqual(repr(t), "'+='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_lappend_apply(self):
        t = LAppend("key", " value2")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], " value2")
        d = {"key": "value1"}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "value1 value2")
        d = {"shortname": "is reserved"}
        LAppend("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_lprepend(self):
        t = LPrepend("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "<=")
        self.assertEqual(repr(t), "'<='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_lprepend_apply(self):
        t = LPrepend("key", "value2 ")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "value2 ")
        d = {"key": "value1"}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "value2 value1")
        d = {"shortname": "is reserved"}
        LPrepend("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_llazyset(self):
        t = LLazySet("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "~=")
        self.assertEqual(repr(t), "'~='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_llazyset_apply(self):
        t = LLazySet("key", "value")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "value")
        d = {"key": "old_value"}
        t.apply_to_dict(d)
        self.assertEqual(d["key"], "old_value")
        d = {"shortname": "is reserved"}
        LLazySet("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_lregexp_set(self):
        t = LRegExpSet("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "?=")
        self.assertEqual(repr(t), "'?='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_lregexp_set_apply(self):
        t = LRegExpSet(r"key_\d", "value")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d, {})
        d = {"key_1": "v1", "key_2": "v2", "key_abc": "v3", "abc": "v4"}
        t.apply_to_dict(d)
        self.assertEqual(d["key_1"], "value")
        self.assertEqual(d["key_2"], "value")
        self.assertEqual(d["key_abc"], "v3")
        self.assertEqual(d["abc"], "v4")
        d = {"shortname": "is reserved"}
        LLazySet("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_lregexp_append(self):
        t = LRegExpAppend("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "?+=")
        self.assertEqual(repr(t), "'?+='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_lregexp_append_apply(self):
        t = LRegExpAppend(r"key_\d", "value")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d, {})
        d = {"key_1": "v1 ", "key_2": "v2 ", "key_abc": "v3 ", "abc": "v4 "}
        t.apply_to_dict(d)
        self.assertEqual(d["key_1"], "v1 value")
        self.assertEqual(d["key_2"], "v2 value")
        self.assertEqual(d["key_abc"], "v3 ")
        self.assertEqual(d["abc"], "v4 ")
        d = {"shortname": "is reserved"}
        LRegExpAppend("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_lregexp_prepend(self):
        t = LRegExpPrepend("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "?<=")
        self.assertEqual(repr(t), "'?<='")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_lregexp_prepend_apply(self):
        t = LRegExpPrepend(r"key_\d", "value")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d, {})
        d = {"key_1": " v1", "key_2": " v2", "key_abc": " v3", "abc": " v4"}
        t.apply_to_dict(d)
        self.assertEqual(d["key_1"], "value v1")
        self.assertEqual(d["key_2"], "value v2")
        self.assertEqual(d["key_abc"], " v3")
        self.assertEqual(d["abc"], " v4")
        d = {"shortname": "is reserved"}
        LRegExpPrepend("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_ldel(self):
        t = LDel("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "del")
        self.assertEqual(repr(t), "'del'")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_ldel_apply(self):
        t = LDel("key_1", "value")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d, {})
        d = {"key_1": "v1", "key_2": "v2"}
        t.apply_to_dict(d)
        self.assertEqual(d, {"key_2": "v2"})
        d = {"shortname": "is reserved"}
        LDel("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")

    def test_lapply_pre_dict(self):
        t = LApplyPreDict("name", {"key": "value"})
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "apply_pre_dict {\"key\": \"value\"}")
        self.assertEqual(repr(t), "'apply_pre_dict {\"key\": \"value\"}'")
        self.assertEqual(t.name, "name")

    def test_lapply_pre_dict_apply(self):
        t = LApplyPreDict("name", {"key_1": "v1", "key_2": "v2"})
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d, {"key_1": "v1", "key_2": "v2"})
        d = {"key_1": "v3", "key_3": "v3"}
        t.apply_to_dict(d)
        self.assertEqual(d, {"key_1": "v1", "key_2": "v2", "key_3": "v3"})
        d = {"shortname": "is reserved"}
        # this is supposed to be safe for overwriting at the times it is invoked
        LApplyPreDict("some", {"shortname": "overwritten"}).apply_to_dict(d)
        self.assertEqual(d["shortname"], "overwritten")

    def test_lupdate_file_map(self):
        t = LUpdateFileMap("filename", "name", "_name_map_file")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "update_file_map")
        self.assertEqual(repr(t), "'update_file_map'")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.filename, "filename")

    def test_lupdate_file_map_apply(self):
        t = LUpdateFileMap("filename", "name", "_name_map_file")
        d = {}
        t.apply_to_dict(d)
        self.assertIn("_name_map_file", d)
        self.assertIn("filename", d["_name_map_file"])
        self.assertEqual(d["_name_map_file"]["filename"], "name")
        d = {"_name_map_file": {"filename": "old_name"}}
        t.apply_to_dict(d)
        self.assertEqual(d["_name_map_file"]["filename"], "name.old_name")

    def test_suffix(self):
        t = Suffix("name", "value")
        self.assertEqual(t.identifier, str(t))
        self.assertEqual(str(t), "suffix value")
        self.assertEqual(repr(t), "'suffix value'")
        self.assertEqual(t.name, "name")
        self.assertEqual(t.value, "value")

    def test_suffix_apply(self):
        t = Suffix("", "s1")
        d = {}
        t.apply_to_dict(d)
        self.assertEqual(d, {})
        d = {"key": "v1", "key_2": "v2", "key_abc": "v3", "abc": "v4"}
        t.apply_to_dict(d)
        self.assertEqual(d[("key", "s1")], "v1")
        self.assertEqual(d[("key_2", "s1")], "v2")
        self.assertEqual(d[("key_abc", "s1")], "v3")
        self.assertEqual(d[("abc", "s1")], "v4")
        d = {"shortname": "is reserved"}
        LLazySet("shortname", "overwritten").apply_to_dict(d)
        self.assertEqual(d["shortname"], "is reserved")


if __name__ == '__main__':
    unittest.main()
