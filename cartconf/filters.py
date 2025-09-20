"""
Filters module.
"""

# TODO: cannot import in a more natural way, see
# https://github.com/PyO3/pyo3/issues/759
# from .cartconf.tokens import Tokens
from .cartconf import filters


#: list of all available manual steps or simply semi-automation tools
__all__ = [
    "Filter",
    "NoOnlyFilter",
    "OnlyFilter",
    "NoFilter",
    "JoinFilter",
    "BlockFilter",
    "Condition",
    "NegativeCondition",
]


Filters = filters.Filters


Filter = Filters.Filter
NoOnlyFilter = Filters.NoOnlyFilter
OnlyFilter = Filters.OnlyFilter
NoFilter = Filters.NoFilter
JoinFilter = Filters.JoinFilter
BlockFilter = Filters.BlockFilter


class Condition(object):
    __slots__ = ["filter", "line", "content", "_inner"]

    # pylint: disable=W0231
    def __init__(self, lfilter: list[list[list[str]]], line: str) -> None:
        self._inner = Filters.Condition(lfilter, line)
        self.filter = lfilter
        self.line = line
        self.content = []

    def __str__(self) -> str:
        return "Condition %s:%s" % (self.filter, self.content)

    def __repr__(self) -> str:
        return "Condition %s:%s" % (self.filter, self.content)

    def match_ctx(self, ctx: list["Label"]) -> bool:
        """
        Check if filter matches in context.

        :param ctx: context to check
        :return: whether filter matches in context
        """
        return self._inner.match_ctx(ctx)

    def might_match(
        self, ctx: list["Label"], descendant_labels: list["Label"]
    ) -> bool:
        """
        Check if filter might match in context.

        :param ctx: context to check
        :param descendant_labels: descendant labels to include
        :return: whether filter might matche in context
        """
        return self._inner.might_match(ctx, descendant_labels)

    def is_irrelevant(
        self, ctx: list["Label"], descendant_labels: list["Label"]
    ) -> bool:
        return self._inner.is_irrelevant(ctx, descendant_labels)

    # pylint: disable=W0613
    def requires_action(
        self, ctx: list["Label"], descendant_labels: list["Label"]
    ) -> bool:
        return self._inner.requires_action(ctx, descendant_labels)

    # pylint: disable=W0613
    def might_pass(
        self,
        failed_ctx: list["Label"],
        ctx: list["Label"],
        descendant_labels: list["Label"],
    ) -> bool:
        return self._inner.might_pass(failed_ctx, ctx, descendant_labels)


class NegativeCondition(Condition):

    # pylint: disable=W0231
    def __init__(self, lfilter: list[list[list[str]]], line: str) -> None:
        super(NegativeCondition, self).__init__(lfilter, line)
        self._inner = Filters.NegativeCondition(lfilter, line)

    def __str__(self) -> str:
        return "NotCond %s:%s" % (self.filter, self.content)

    def __repr__(self) -> str:
        return "NotCond %s:%s" % (self.filter, self.content)
