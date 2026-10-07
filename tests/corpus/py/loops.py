"""Loops.

Defect class 7: a Java `for` header gained a doubled semicolon and a
duplicated condition. Class 9: the Go output called `fmt.Println` with no
`import "fmt"`.
"""


def count_to(n):
    total = 0
    i = 0
    while i < n:
        total = total + i
        i = i + 1
    return total


def each(items):
    for item in items:
        print(item)


def labelled(rows):
    for row in rows:
        for cell in row:
            if cell is None:
                continue
            print(cell)


def with_else(items):
    for item in items:
        if item:
            break
    else:
        print("empty")


def enumerate_pairs(items):
    for index, item in enumerate(items):
        print(index, item)
