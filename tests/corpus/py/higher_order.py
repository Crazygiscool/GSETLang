"""Higher-order functions.

Functions taking functions, returning functions, and closures over loop
variables.
"""


def apply(fn, value):
    return fn(value)


def compose(outer, inner):
    return lambda value: outer(inner(value))


def adder(amount):
    def add(value):
        return value + amount

    return add


def repeater(times):
    def repeat(value):
        return value * times

    return repeat


def map_all(values, fn):
    return [apply(fn, value) for value in values]


def reduce_all(values, fn, initial):
    accumulator = initial
    for value in values:
        accumulator = fn(accumulator, value)
    return accumulator


def filter_all(values, predicate):
    return [value for value in values if predicate(value)]
