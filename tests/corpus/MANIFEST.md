# Python corpus manifest
#
# Which file stands in for which defect class from `tests/baseline/README.md`.
#
# The classes were found in the Go implementation's output for the `.gset`
# fixtures. The corpus is written in real Python, because the rewrite retires
# `.gset`, so each class is represented by the real-language construct that
# provokes the same failure. A file may cover more than one class.
#
# This list is asserted by `the_corpus_covers_every_audit_defect_class` in
# `crates/gset-frontend/src/python/mod.rs`, so a class cannot be quietly dropped:
# renaming or deleting a file here fails the build.

class 1: only the first top-level if emitted: module_shape.py
class 2: struct and enum vanished: declarations.py
class 3: ?? has no Go mapping: null_handling.py
class 4: ?? became || in Ruby: null_handling.py
class 5: comprehension passed through: comprehensions.py
class 6: literal newline in a block lambda: lambdas.py
class 7: malformed loop header: loops.py
class 8: interface{} and an undeclared name: literals.py
class 9: missing import: imports.py
class 10: integer literal saturates: numeric_literals.py
class 11: name replaced by nil: declaration_after_if.py
class 12: untyped parameter: returns.py
class 13: void method returning a value: returns.py
class 14: function swallows the module: returns.py
class 15: no trailing newline in output: all

# Constructs the source language adds that the old emitter never saw.
decorators survive as written: decorators.py
f-strings keep their shape: fstrings.py
comprehensions keep their shape: comprehensions.py
generators and async stay distinct: async_flow.py
context managers keep their exit order: context_managers.py
exceptions: exceptions.py
destructuring: destructuring.py
operators: operators.py
classes and methods: classes.py
higher-order functions: higher_order.py
parameter forms: arguments.py
annotations are preserved: typed.py