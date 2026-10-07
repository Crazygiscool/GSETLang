"""Lambdas.

Defect class 6: a block lambda emitted into JavaScript carried a literal `\n`
escape, collapsing the body onto one line and swallowing the code after it.
"""


double = lambda x: x * 2
add = lambda a, b: a + b


def apply(value, fn):
    return fn(value)


def run(value):
    return apply(value, double)


def keyfn(pair):
    return pair[0]


def sorted_pairs(pairs):
    return sorted(pairs, key=keyfn)
