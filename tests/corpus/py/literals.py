"""Literals and annotations.

Defect class 8: `[]interface{}{1, 2}` reached Go and `xs` was never declared.
Class 12: a parameter with no type. Both are what an untyped dynamic source
looks like when a target has no equivalent.
"""

from typing import Dict, List, Optional

EMPTY: List[int] = []
NAMES: Dict[str, int] = {}
MIXED = [1, "two", 3.0, True, None]
NESTED = [[1, 2], [3, 4], []]


def annotated(values: List[int], table: Dict[str, int]) -> int:
    total = 0
    for value in values:
        total += value
    return total


def optional(value: Optional[int]) -> Optional[int]:
    return value


def untyped(a, b=2, *rest, **options):
    return a


def joined(values):
    return ",".join(str(value) for value in values)
