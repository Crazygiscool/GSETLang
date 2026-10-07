"""Decorators.

Decorators must survive into the IR as written rather than being desugared into
wrapper calls: the order is load-bearing, and mapping one onto a native
construct is the backend's decision, not the frontend's.
"""

import functools
import logging

log = logging.getLogger(__name__)


def staticmethod_like(subject):
    """Bound here so the class decorator below resolves."""
    return subject


@log.debug
def traced(value):
    return value


@functools.lru_cache(maxsize=None)
def memoised(value):
    return value * 2


@staticmethod_like
class Decorated:
    @property
    def size(self):
        return 10

    @size.setter
    def size(self, value):
        self._size = value

    @functools.cached_property
    def computed(self):
        return 42

    @classmethod
    def build(cls):
        return cls()


def stacked():
    return 1
