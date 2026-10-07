"""Declarations after control flow.

Defect class 11: `x = 1 ?? 2` after an `if` became `nil = 1 ?? 2`, and the
name was never declared at all.
"""


def run(value):
    if value > 0:
        print("positive")
    result = value * 2
    print(result)
    return result


def flag(ready):
    if ready:
        chosen = "fast"
    else:
        chosen = "slow"
    return chosen


def conditional_binding(items):
    if items:
        found = items[0]
    found_count = len(items)
    return found_count
