"""Exceptions.

`try`/`except`/`else`/`finally`, bare and named handlers, re-raise, and
raising a new exception from a caught one.
"""


def parse(text):
    try:
        return int(text)
    except ValueError:
        return 0
    except (TypeError, KeyError) as error:
        print(error)
        raise
    finally:
        print("done")


def else_branch(value):
    try:
        result = value * 2
    except ValueError:
        return None
    else:
        return result


def chained():
    try:
        risky()
    except ValueError as error:
        raise RuntimeError("wrapped") from error


def finally_runs():
    handled = False
    try:
        risky()
        handled = True
    except Exception:
        pass
    finally:
        handled = False
    return handled


def risky():
    raise ValueError("boom")
