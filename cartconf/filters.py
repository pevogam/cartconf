"""
Filters module.
"""

#: list of all available manual steps or simply semi-automation tools
__all__ = ["Filter", "NoOnlyFilter", "OnlyFilter", "NoFilter", "JoinFilter",
           "BlockFilter", "Condition", "NegativeCondition"]


# Filter must inherit from object (otherwise type() won't work)
class Filter(object):
    __slots__ = ["filter"]

    def __init__(self, lfilter):
        self.filter = lfilter
        # print self.filter

    def match(self, ctx, ctx_set):
        for word in self.filter:  # Go through ,
            for block in word:    # Go through ..
                if _match_adjacent(block, ctx, ctx_set) != len(block):
                    break
            else:
                # print "Filter pass: %s ctx: %s" % (self.filter, ctx)
                return True       # All match
        return False

    def might_match(self, ctx, ctx_set, descendant_labels):
        # There is some possibility to match in children blocks.
        for word in self.filter:
            for block in word:
                if not _might_match_adjacent(block, ctx, ctx_set,
                                             descendant_labels):
                    break
            else:
                return True
        # print "Filter not pass: %s ctx: %s" % (self.filter, ctx)
        return False


class NoOnlyFilter(Filter):
    __slots__ = ("line")

    def __init__(self, lfilter, line):
        super(NoOnlyFilter, self).__init__(lfilter)
        self.line = line

    def __eq__(self, o):
        if isinstance(o, self.__class__):
            if self.filter == o.filter:
                return True

        return False


class OnlyFilter(NoOnlyFilter):
    # pylint: disable=W0613

    def is_irrelevant(self, ctx, ctx_set, descendant_labels):
        # Matched in this tree.
        return self.match(ctx, ctx_set)

    def requires_action(self, ctx, ctx_set, descendant_labels):
        # Impossible to match in this tree.
        return not self.might_match(ctx, ctx_set, descendant_labels)

    def might_pass(self, failed_ctx, failed_ctx_set, ctx, ctx_set,
                   descendant_labels):
        for word in self.filter:
            for block in word:
                if (_match_adjacent(block, ctx, ctx_set) >
                        _match_adjacent(block, failed_ctx, failed_ctx_set)):
                    return self.might_match(ctx, ctx_set, descendant_labels)
        return False

    def __str__(self):
        return "Only %s" % (self.filter)

    def __repr__(self):
        return "Only %s" % (self.filter)


class NoFilter(NoOnlyFilter):

    def is_irrelevant(self, ctx, ctx_set, descendant_labels):
        return not self.might_match(ctx, ctx_set, descendant_labels)

    # pylint: disable=W0613
    def requires_action(self, ctx, ctx_set, descendant_labels):
        return self.match(ctx, ctx_set)

    # pylint: disable=W0613
    def might_pass(self, failed_ctx, failed_ctx_set, ctx, ctx_set,
                   descendant_labels):
        for word in self.filter:
            for block in word:
                if (_match_adjacent(block, ctx, ctx_set) <
                        _match_adjacent(block, failed_ctx, failed_ctx_set)):
                    return not self.match(ctx, ctx_set)
        return False

    def __str__(self):
        return "No %s" % (self.filter)

    def __repr__(self):
        return "No %s" % (self.filter)


class JoinFilter(NoOnlyFilter):

    def __str__(self):
        return "Join %s" % (self.filter)

    def __repr__(self):
        return "Join %s" % (self.filter)


class BlockFilter(object):
    __slots__ = ["blocked"]

    def __init__(self, blocked):
        self.blocked = blocked

    def apply_to_dict(self, d):
        pass


class Condition(NoFilter):
    __slots__ = ["content"]

    # pylint: disable=W0231
    def __init__(self, lfilter, line):
        super(Condition, self).__init__(lfilter, line)
        self.content = []

    def __str__(self):
        return "Condition %s:%s" % (self.filter, self.content)

    def __repr__(self):
        return "Condition %s:%s" % (self.filter, self.content)


class NegativeCondition(OnlyFilter):
    __slots__ = ["content"]

    # pylint: disable=W0231
    def __init__(self, lfilter, line):
        super(NegativeCondition, self).__init__(lfilter, line)
        self.content = []

    def __str__(self):
        return "NotCond %s:%s" % (self.filter, self.content)

    def __repr__(self):
        return "NotCond %s:%s" % (self.filter, self.content)


# Helpers for all filters
def _match_adjacent(block, ctx, ctx_set):
    """
    It try to match as many blocks as possible from context.

    :return: Count of matched blocks.
    """
    if block[0] not in ctx_set:
        return 0
    if len(block) == 1:
        return 1                          # First match and length is 1.
    if block[1] not in ctx_set:
        return int(ctx[-1] == block[0])   # Check match with last from ctx.
    k = 0
    i = ctx.index(block[0])
    while i < len(ctx):                   # Try to  match all of blocks.
        if k > 0 and ctx[i] != block[k]:  # Block not match
            i -= k - 1
            k = 0                         # Start from first block in next ctx.
        if ctx[i] == block[k]:
            k += 1
            if k >= len(block):           # match all of blocks
                break
            if block[k] not in ctx_set:   # block in not in whole ctx.
                break
        i += 1
    return k


def _might_match_adjacent(block, ctx, ctx_set, descendant_labels):
    matched = _match_adjacent(block, ctx, ctx_set)
    for elem in block[matched:]:        # Try to find rest of blocks in subtree
        if elem not in descendant_labels:
            # print "Can't match %s, ctx %s" % (block, ctx)
            return False
    return True
