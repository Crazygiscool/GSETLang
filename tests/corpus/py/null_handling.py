"""Optional values and coalescing.

Defect classes 3 and 4: `??` has no Go mapping and silently became `||` in
Ruby. Python has no `??` either; the equivalents are `or`, `if x is None`
and the walrus operator, and each means something slightly different.
"""


def pick(maybe):
    value = maybe or "default"
    return value


def first_or_none(items, index):
    if index < len(items):
        return items[index]
    return None


def walrus(items):
    if (length := len(items)) > 3:
        return length
    return 0


def coalesce_chain(a, b, c):
    return a or b or c


def explicit_none(a, b):
    return b if a is None else a
