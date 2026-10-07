"""Context managers.

`with` and `as`, including several resources at once. A target with `defer`
needs the exit order preserved.
"""


def read_file(path):
    with open(path) as handle:
        return handle.read()


def read_two(first_path, second_path):
    with open(first_path) as first, open(second_path) as second:
        return first.read(), second.read()


def writing(path, text):
    with open(path, "w") as handle:
        handle.write(text)


def conditional(path):
    if path:
        with open(path) as handle:
            return handle.read()
    return None
