"""f-strings.

Interpolation must keep its source shape. A backend may emit concatenation, a
format function or a template literal, but it must not have to re-parse the
literal to discover where the holes were.
"""


def greet(name):
    return f"hello {name}"


def pair(key, value):
    return f"{key}={value}"


def formatted(flag):
    return f"{value:.2f}" if flag else str(value)


def nested(name):
    return f"outer {f'inner {name}'}"


def multi():
    return f"{a}-{b}-{c}"


def with_literal_braces():
    return f"{{literal}} {value}"


value = 1
