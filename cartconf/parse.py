#!/usr/bin/python

"""
Main runnable module.
"""

import optparse
import logging

from .parser import Parser


LOG = logging.getLogger("avocado." + __name__)
options = None


def print_dicts_default(options, dicts):
    """Print dictionaries in the default mode"""
    for count, dic in enumerate(dicts):
        if options.fullname:
            print("dict %4d:  %s" % (count + 1, dic["name"]))
        else:
            print("dict %4d:  %s" % (count + 1, dic["shortname"]))
        if options.contents:
            keys = list(dic.keys())
            keys.sort()
            for key in keys:
                print("    %s = %s" % (key, dic[key]))


# pylint: disable=W0613
def print_dicts_repr(options, dicts):
    import pprint

    print("[")
    for dic in dicts:
        print("%s," % (pprint.pformat(dic)))
    print("]")


def print_dicts(options, dicts):
    if options.repr_mode:
        print_dicts_repr(options, dicts)
    else:
        print_dicts_default(options, dicts)


if __name__ == "__main__":
    parser = optparse.OptionParser(
        "usage: %prog [options] filename "
        "[extra code] ...\n\nExample:\n\n    "
        '%prog tests.cfg "only my_set" "no qcow2"'
    )
    parser.add_option(
        "-v",
        "--verbose",
        dest="debug",
        action="store_true",
        help="include debug messages in console output",
    )
    parser.add_option(
        "-f",
        "--fullname",
        dest="fullname",
        action="store_true",
        help="show full dict names instead of short names",
    )
    parser.add_option(
        "-c",
        "--contents",
        dest="contents",
        action="store_true",
        help="show dict contents",
    )
    parser.add_option(
        "-r",
        "--repr",
        dest="repr_mode",
        action="store_true",
        help="output parsing results Python format",
    )
    parser.add_option(
        "-d",
        "--defaults",
        dest="defaults",
        action="store_true",
        help="use only default variant of variants if there" " is some",
    )
    parser.add_option(
        "-e",
        "--expand",
        dest="expand",
        type="string",
        help="list of vartiant which should be expanded when"
        ' defaults is enabled.  "name, name, name"',
    )
    parser.add_option(
        "-s",
        "--skip-dups",
        dest="skipdups",
        default=True,
        action="store_false",
        help="Don't drop variables with different suffixes and same val",
    )

    options, args = parser.parse_args()
    if not args:
        parser.error("filename required")

    if options.debug:
        LOG.setLevel(logging.DEBUG)

    expand = []
    if options.expand:
        expand = [x.strip() for x in options.expand.split(",")]
    c = Parser(
        args[0], defaults=options.defaults, expand_defaults=expand, debug=options.debug
    )
    for s in args[1:]:
        c.parse_string(s)

    if options.debug:
        print(c.node.dump(0, True))

    dicts = c.get_dicts(skipdups=options.skipdups)
    print_dicts(options, dicts)
