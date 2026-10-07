"""Comprehensions.

Defect class 5: Python comprehension syntax reached Go output unchanged and
did not parse. All four forms are here because each maps differently.
"""


def squares(nums):
    return [n * n for n in nums]


def evens(nums):
    return [n for n in nums if n % 2 == 0]


def unique_lengths(words):
    return {len(word) for word in words}


def invert(pairs):
    return {key: value for key, value in pairs}


def counted(nums):
    return (n for n in nums)


def nested_pairs(rows):
    return [[cell for cell in row] for row in rows]


def with_two_clauses(pairs):
    return [(a, b) for a, b in pairs if a < b]
