# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
#
# See LICENSE for more details.
#
# Copyright: Red Hat Inc. 2016-2017
# Author: Lukas Doktor <ldoktor@redhat.com>

import sys
from typing import Generator

from avocado.core import exit_codes
from avocado.core.output import LOG_UI
from avocado.core.plugin_interfaces import CLI, Init, Varianter
from avocado.core.tree import TreeNode
from avocado.core.settings import settings

from cartconf.parser import Parser


class CartConfInit(Init):
    """CartConf initialization plugin."""

    name = "cartconf"
    description = "CartConf initialization plugin"

    def initialize(self) -> None:
        help_msg = (
            "Location of one or more Cartesian config (.cfg) "
            "FILE(s) (order dependent)"
        )
        settings.register_option(
            section=self.name, key="files", default=[], key_type=list, help_msg=help_msg
        )

        help_msg = "Only filters to restrict variants"
        settings.register_option(
            section=self.name,
            key="only",
            default=[],
            key_type=list,
            help_msg=help_msg,
        )

        help_msg = "No filters to restrict variants"
        settings.register_option(
            section=self.name, key="no", default=[], help_msg=help_msg
        )

        help_msg = "Overwrite key=value pairs for the final parameter dictionary"
        settings.register_option(
            section=self.name,
            key="overwrite",
            default=[],
            help_msg=help_msg,
            key_type=list,
        )


class CartConfCLI(CLI):
    """Defines arguments for CartConf plugin."""

    name = "cartconf"
    description = "CartConf options for the 'run' subcommand"

    def configure(self, parser: "argparse.ArgumentParser") -> None:
        """
        Configures "run" and "variants" subparsers.

        :param parser: argparse parser object
        """
        for name in ("run", "variants"):
            subparser = parser.subcommands.choices.get(name, None)
            if subparser is None:
                continue
            agroup = subparser.add_argument_group("cartconf options")
            settings.add_argparser_to_option(
                namespace=f"{self.name}.{'files'}",
                parser=agroup,
                long_arg="--cartconf",
                short_arg="-C",
                metavar="FILE",
                nargs="+",
                allow_multiple=True,
            )

            settings.add_argparser_to_option(
                namespace=f"{self.name}.{'only'}",
                parser=agroup,
                long_arg="--only",
                nargs="+",
                allow_multiple=True,
                metavar="ONLY",
            )

            settings.add_argparser_to_option(
                namespace=f"{self.name}.{'no'}",
                parser=agroup,
                long_arg="--no",
                nargs="+",
                allow_multiple=True,
                metavar="NO",
            )

            settings.add_argparser_to_option(
                namespace=f"{self.name}.{'overwrite'}",
                parser=agroup,
                long_arg="--overwrite",
                short_arg="-o",
                nargs="+",
                allow_multiple=True,
                metavar="PATH_KEY_NODE",
            )

    def run(self, config: "argparse.Namespace") -> None:
        """
        The CartConf varianter plugin handles these.

        :param config: argparse namespace object
        """


class CartConf(Parser, Varianter):
    """Processes the options into varianter plugin."""

    name = "cartconf"
    description = "Varianter plugin to parse Cartesian configuration to params"

    def initialize(self, config: dict[str, str]) -> None:
        subcommand = config.get("subcommand")

        config_files = config.get("cartconf.files")
        for config_file in config_files:
            try:
                self.parse_file(config_file)
            except IOError as details:
                error_msg = f"{details.strerror} : {details.filename}"
                LOG_UI.error(error_msg)
                if subcommand == "run":
                    sys.exit(exit_codes.AVOCADO_JOB_FAIL)
                else:
                    sys.exit(exit_codes.AVOCADO_FAIL)

        only_filters = config.get("cartconf.only")
        for only_filter in only_filters:
            self.only_filter(only_filter)

        no_filters = config.get("cartconf.no")
        for no_filter in no_filters:
            self.no_filter(no_filter)

        for overwrite in config.get("cartconf.overwrite"):
            self.assign(*overwrite.split("="))

    def __iter__(self) -> Generator[dict[str, str], None, None]:
        """
        Yields all variants.

        The variant is defined as dictionary with at least:
         * variant_id - name of the current variant
         * variant - AvocadoParams-compatible variant (usually a list)
         * paths - default path(s)

        :returns: generator of variants
        """
        for d in self.get_dicts():
            yield {
                "variant_id": d["name"],
                "variant": TreeNode(d["name"], d),
                "paths": ["/"],
            }

    def __len__(self) -> int:
        """Report number of variants."""
        return len(list(self.get_dicts()))

    def to_str(self, summary: int, variants: int, **kwargs) -> str:
        """
        Return human readable representation.

        The summary/variants accepts verbosity where 0 means silent and
        maximum is up to the plugin.

        :param summary: How verbose summary to output
        :param variants: How verbose list of variants to output
        :param kwargs: Other free-form arguments
        :returns: string representation of the variants
        """
        output = ""
        for i, d in enumerate(self.get_dicts()):
            if summary > 0:
                output += "dict %4d:  %s" % (i + 1, d["name"]) + "\n"
            else:
                output += "dict %4d:  %s" % (i + 1, d["shortname"]) + "\n"
            if variants > 0:
                keys = list(d.keys())
                keys.sort()
                for key in keys:
                    output += "    %s = %s" % (key, d[key]) + "\n"
        return output
