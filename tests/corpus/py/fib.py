"""Fibonacci with annotations.

The milestone 1 gate: `gset transpile tests/corpus/py/fib.py --to go` must emit
Go that passes `gofmt` and `go vet`. Annotated deliberately, because the IR does
not infer types yet, and guessing a dynamic parameter's type is exactly the
defect class the rewrite exists to remove.
"""


def fib(n: int) -> int:
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


print(fib(10))
