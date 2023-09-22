"""
Exceptions module.
"""

class ParserError(Exception):

    def __init__(self, msg, line=None, filename=None, linenum=None):
        Exception.__init__(self)
        self.msg = msg
        self.line = line
        self.filename = filename
        self.linenum = linenum

    def __str__(self):
        if self.line:
            return "%s: %r (%s:%s)" % (self.msg, self.line,
                                       self.filename, self.linenum)
        else:
            return "%s (%s:%s)" % (self.msg, self.filename, self.linenum)


class LexerError(ParserError):
    pass


class MissingIncludeError(Exception):

    def __init__(self, line, filename, linenum):
        Exception.__init__(self)
        self.line = line
        self.filename = filename
        self.linenum = linenum

    def __str__(self):
        return ("%r (%s:%s): file does not exist or it's not a regular "
                "file" % (self.line, self.filename, self.linenum))
