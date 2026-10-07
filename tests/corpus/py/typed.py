"""Annotated code.

Where the source states types the IR should carry them, so a backend does not
have to re-derive what was already written down.
"""

from typing import Dict, List, Optional, Tuple, Union


def total(values: List[int]) -> int:
    return sum(values)


def index(table: Dict[str, int], key: str) -> Optional[int]:
    return table.get(key)


def split(pair: Tuple[int, int]) -> Tuple[int, int]:
    return pair


def either(value: Union[int, str]) -> str:
    return str(value)


def annotated_defaults(count: int = 0, ratio: float = 1.5) -> float:
    return ratio * count


def nested_table() -> Dict[str, List[int]]:
    return {}


def coerced(value: Union[int, float]) -> float:
    return float(value)
