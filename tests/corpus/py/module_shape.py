"""Module-level statement shape.

Defect classes 1 and 14: a second top-level `if` lost its header in the Go
implementation, and the first statement after a function definition was
absorbed into that function.
"""


def first():
    return 1


x = 2

if x > 1:
    first()


if x > 2:
    second()


y = first() + x
