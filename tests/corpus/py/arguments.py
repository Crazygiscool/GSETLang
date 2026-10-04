"""Parameter forms.

Defaults, keyword arguments, `*args`, `**kwargs` and keyword-only parameters.
"""

DEFAULT = 10


def simple(a, b):
    return a + b


def defaulted(a, b=DEFAULT, *rest):
    return a + b


def keyword_only(a, *, strict=False):
    return strict and a > 0


def variadic(*args, **kwargs):
    return args, kwargs


def call_with_names(fn):
    return fn(a=1, b=2)


def unpacking_call(fn, values, options):
    return fn(*values, **options)


def defaults_mutating(a, b=[]):
    return b.append(a)
