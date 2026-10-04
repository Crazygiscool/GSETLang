"""Return shapes.

Defect class 12: an untyped Go parameter. Class 13: a Java `void` method
that returns a value. Class 14: the rest of the module ended up inside the
function.
"""


def pick(n):
    if n > 0:
        return 1
    return 0


def no_return_ever(n):
    if n > 0:
        print(n)


def bare_return(ok):
    if ok:
        return
    print("failed")


def branch_returns(n):
    if n > 10:
        return "big"
    elif n > 5:
        return "medium"
    elif n > 0:
        return "small"
    return "non-positive"


def implicit_none(n):
    if n:
        return n


after_first = 1
print(pick(5))
