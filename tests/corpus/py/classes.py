"""Classes.

Inheritance, multiple bases, instance and class state, and the Python-only
concepts a target has to translate rather than copy.
"""

from abc import ABC, abstractmethod


class Shape:
    sides = 0

    def __init__(self, name):
        self.name = name

    def describe(self):
        return self.name

    @classmethod
    def origin(cls):
        return cls.__name__


class Polygon(Shape, ABC):
    def __init__(self, name, sides):
        super().__init__(name)
        self.sides = sides

    @abstractmethod
    def area(self):
        pass

    def is_closed(self):
        return True


class Square(Polygon):
    def __init__(self, side):
        super().__init__("square", 4)
        self.side = side

    def area(self):
        return self.side * self.side


def build(side):
    shape = Square(side)
    return shape.area(), shape.describe()


def uses_class_state():
    return Shape.sides, Shape.origin()
