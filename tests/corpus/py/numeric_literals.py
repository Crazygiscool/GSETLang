"""Numeric literals.

Defect class 10: a literal too large for int64 saturated at 9223372036854775807
and the overflow was discarded with no diagnostic. Python has arbitrary
precision, so this file holds values no fixed-width target can hold.
"""

BIG = 12345678901234567890
BIGGER = 2**64
NEGATIVE = -170141183460469231731687303715884105728
SMALL = 42
FLOATY = 3.141592653589793
TINY = 1e-300
HUGE = 1e300
HEXED = 0xDEADBEEF
OCTAL = 0o755
BINARY = 0b1011
UNDERSCORED = 1_000_000


def arithmetic(a, b):
    return a * b + a // b - a % b


def shifts(flag):
    return flag << 2 | flag >> 1


def mixed():
    return BIG + SMALL, FLOATY * BIG, HEXED | OCTAL
