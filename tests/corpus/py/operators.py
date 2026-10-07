"""Operator coverage.

Chained comparisons, the boolean operators, membership, identity, slicing and
bitwise work. Several of these have no direct equivalent in some targets, which
is precisely why the emitter must not pattern-match on source text.
"""


def chained(n):
    return 0 < n < 10


def logical(a, b, c):
    return a and b or c


def negated(a, b):
    return not (a and not b)


def membership(item, items):
    return item in items


def identity(a, b):
    return a is b


def slicing(items):
    return items[1:3] + items[:2] + items[::2]


def bitwise(a, b):
    return (a | b) & (a ^ b) >> 1


def ternary(flag):
    return 1 if flag else 0


def chained_calls(value):
    return value.strip().lower().split(",")
