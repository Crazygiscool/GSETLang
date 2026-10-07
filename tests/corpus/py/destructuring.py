"""Destructuring.

Tuple unpacking, starred targets, nested targets and unpacking in a `for`.
"""


def swap(a, b):
    a, b = b, a
    return a, b


def head_and_tail(items):
    first, *rest = items
    return first, rest


def nested_pair(pair):
    (left, right), third = pair, 3
    return left, right, third


def unpack_each(pairs):
    for key, value in pairs:
        print(key, value)


def swap_dict(dictionary):
    a, b = dictionary["a"], dictionary["b"]
    return b, a


def ignored(items):
    _, second, _ = items
    return second
