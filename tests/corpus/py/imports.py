"""Import forms.

Defect class 9: Go output called `fmt.Println` with no import, because the
import decision was a string search over the AST rather than a value.

All five Python import forms are here, including the star import that has no
safe equivalent in most targets.
"""

from __future__ import annotations

import os
import os.path
import sys as system
from collections import OrderedDict
from collections import defaultdict as fallback, deque
from . import sibling
from .relative import thing
TIMEOUT = 30


def paths():
    return os.path.join("a", "b")


def ordered():
    return OrderedDict()


def queue(items):
    return deque(items)
