"""Declarations.

Defect class 2: a struct, an enum and a class in one input produced output
containing only the class. The other two disappeared with no warning.

Python spells the same three shapes with `Enum`, `dataclass` and a plain
class.
"""

from dataclasses import dataclass
from enum import Enum


class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3


@dataclass
class Point:
    x: int
    y: int


class Widget:
    def __init__(self, point):
        self.point = point

    def value(self):
        return self.point.x
