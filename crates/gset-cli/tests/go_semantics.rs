//! Semantic equivalence: the Go we emit has to print what Python prints.
//!
//! The corpus gate proves the Go compiles and the golden proves the emitter is
//! deterministic. Neither says the program still *means* what it meant: `//`
//! rounded the wrong way, a `dict.get` invented a key, a loop's `else` ran when
//! it should not, and every one of those emits clean, gofmt-clean, vet-clean Go
//! that computes a different answer.
//!
//! So each case here is run twice — once by `python3`, once by `go run` — and the
//! two stdouts have to match. A construct the backend cannot express is caught by
//! the corpus gate instead, so a case here is always one that should work.
//!
//! What is compared is the value each program printed, not the spelling: Go
//! writes `true` where Python writes `True`, and a whole float as `3` where
//! Python writes `3.0`. [`normalize`] reads those as the same value, and the
//! cases print their containers through `", ".join(...)` because no two
//! languages agree on how a list looks.

use std::path::Path;
use std::process::{Command, Stdio};

use gset_cli::transpile;

/// One program, in the language it started in.
struct Case {
    name: &'static str,
    source: &'static str,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "floor_division_rounds_down",
            source: "\
print(7 // 2)
print(-7 // 2)
print(7 // -2)
print(-7 // -2)
print(7.5 // 2)
print(0 // 5)
",
        },
        Case {
            name: "power_returns_int_for_int_operands",
            source: "\
print(2 ** 10)
print(2 ** 0)
print(3 ** 3)
print(2 ** -1)
",
        },
        Case {
            name: "membership",
            source: "\
xs = [1, 2, 3]
print(2 in xs)
print(9 in xs)
print(2 not in xs)
print(\"ell\" in \"hello\")
print(\"xyz\" in \"hello\")
d = {\"a\": 1}
print(\"a\" in d)
print(\"b\" in d)
",
        },
        Case {
            name: "string_methods",
            source: "\
s = \"  Hello World  \"
print(s.upper())
print(s.lower())
print(s.strip())
print(s.strip().split(\" \")[1])
print(\"-\".join([\"a\", \"b\", \"c\"]))
print(\"a-b-c\".replace(\"-\", \"+\"))
print(\"hello\".startswith(\"he\"))
print(\"hello\".endswith(\"lo\"))
print(\"hello\".find(\"l\"))
print(\"hello\".rfind(\"l\"))
print(\"hello\".count(\"l\"))
print(len(\"one\\ntwo\\nthree\".splitlines()))
",
        },
        Case {
            name: "list_methods",
            source: "\
xs = [3, 1, 2]
xs.append(4)
print(\", \".join([str(x) for x in xs]))
xs.extend([5, 6])
print(\", \".join([str(x) for x in xs]))
xs.insert(0, 0)
print(\", \".join([str(x) for x in xs]))
xs.remove(0)
print(\", \".join([str(x) for x in xs]))
xs.reverse()
print(\", \".join([str(x) for x in xs]))
xs.sort()
print(\", \".join([str(x) for x in xs]))
print(xs.index(3))
print(xs.count(2))
print(xs.pop())
print(\", \".join([str(x) for x in xs]))
xs.clear()
print(len(xs))
",
        },
        Case {
            name: "dict_methods",
            source: "\
d = {\"a\": 1, \"b\": 2}
print(d[\"a\"])
print(d.get(\"a\"))
print(d.get(\"z\"))
print(d.get(\"z\", \"fallback\"))
print(\", \".join([str(k) for k in sorted(d.keys())]))
print(\", \".join([str(v) for v in sorted(d.values())]))
print(\", \".join(sorted([str(pair[0]) for pair in d.items()])))
d.update({\"c\": 3})
print(\", \".join([str(k) for k in sorted(d.keys())]))
print(d[\"a\"], d[\"b\"], d[\"c\"])
",
        },
        Case {
            name: "literals_and_slices",
            source: "\
xs = [1, 2, 3, 4, 5]
print(\", \".join([str(x) for x in xs[1:3]]))
print(\", \".join([str(x) for x in xs[:2]]))
print(\", \".join([str(x) for x in xs[3:]]))
print(\", \".join([str(x) for x in xs[:]]))
print(\", \".join([str(x) for x in xs[0:4]]))
print(\", \".join([str(x) for x in xs[-2:]]))
print(len({\"a\": 1, \"b\": 2}))
",
        },
        Case {
            name: "conditional_expression",
            source: "\
flag = False
print(1 if flag else 2)
print(\"yes\" if not flag else \"no\")
for n in [1, 5]:
    print(\"small\" if n < 3 else \"big\")
",
        },
        Case {
            name: "comprehensions",
            source: "\
nums = [1, 2, 3, 4]
print(\", \".join([str(n * n) for n in nums]))
print(\", \".join([str(n) for n in nums if n % 2 == 0]))
print(len({n: n * 2 for n in nums}))
print({n: n * 2 for n in nums}[3])
print(len([[cell for cell in row] for row in [[1, 2], [3]]]))
print(\", \".join([str(a * b) for a in [1, 2] for b in [10, 20]]))
",
        },
        Case {
            name: "loop_else_runs_without_break",
            source: "\
for n in range(4):
    if n == 9:
        break
else:
    print(\"completed\")

for n in range(9):
    if n == 2:
        print(\"found\", n)
        break
else:
    print(\"completed\")
",
        },
        Case {
            name: "walrus_binds_once",
            source: "\
xs = [1, 2, 3]
if (n := len(xs)) > 2:
    print(\"long\", n)
while (line := \"next\") == \"next\":
    print(line)
    break
",
        },
        Case {
            name: "interpolated_strings",
            source: "\
name = \"world\"
count = 3
print(f\"hello {name}\")
print(f\"{name}={count}\")
print(f\"nested {f'inner {name}'}\")
print(f\"literal {{braces}} {count}\")
print(f\"{name.upper()} has {count} letters\")
",
        },
        Case {
            name: "falsy_and_truthy_values",
            source: "\
maybe = None
print(maybe or \"default\")
print(maybe if maybe else \"none\")
",
        },
        Case {
            name: "every_branch_of_an_elif_chain_runs",
            source: "\
def label(n):
    if n > 10:
        return \"big\"
    elif n > 5:
        return \"medium\"
    elif n > 0:
        return \"small\"
    else:
        return \"non-positive\"

for n in [11, 7, 2, 0]:
    print(label(n))
",
        },
        Case {
            name: "reaching_the_end_returns_none",
            source: "\
def maybe(n):
    if n:
        return n

print(maybe(5) is None)
print(maybe(0) is None)
print(maybe(5))
print(maybe(0))
",
        },
        Case {
            name: "ints_and_floats_mix_like_python",
            source: "\
count = 3
ratio = 1.5
print(count + ratio)
print(count * ratio)
print(ratio - count)
print(count < ratio)
print(ratio < count)
print(count == 3)
print(count == 3.0)
print(count != 3.5)
print(3 / 2)
print(7 / 2)
print(count / 2)
print(-7 / 2)
",
        },
        Case {
            name: "containers_compare_by_value",
            source: "\
xs = [1, 2]
ys = [1, 2]
zs = [2, 1]
print(xs == ys)
print(xs == zs)
print(xs != zs)
a = {\"k\": [1, 2]}
b = {\"k\": [1, 2]}
print(a == b)
c = {\"k\": [1, 3]}
print(a == c)
",
        },
        Case {
            name: "untyped_arithmetic_answers_at_runtime",
            source: "\
def twice(value):
    return value * 2

def offset(value):
    return value - 1

def remainder(value):
    return value % 3

def join(left, right):
    return left + right

for argument in (3, 2.5, True):
    print(twice(argument))
    print(offset(argument))
    print(remainder(argument))
print(join(1, 2))
print(join(1.5, 2))
print(join(\"a\", \"b\"))
print(len(join([1], [2])))
print(join([1], [2])[0] + 10)
print(remainder(-7))
",
        },
        Case {
            name: "a_name_bound_by_every_branch_outlives_the_branch",
            source: "\
def label(ready):
    if ready:
        chosen = \"fast\"
    else:
        chosen = \"slow\"
    return chosen

def total(items):
    count = 0
    for item in items:
        count = count + item
    if count > 2:
        result = \"many\"
    else:
        result = \"few\"
    return result

print(label(True))
print(label(False))
print(total([1, 2]))
print(total([1, 2, 3, 4]))
",
        },
        Case {
            name: "conversions_follow_python",
            source: "\
def as_float(value):
    return float(value)

def as_int(value):
    return int(value)

print(as_float(3))
print(as_float(2.5))
print(as_float(\"2.5\"))
print(as_float(True))
print(as_int(3.9))
print(as_int(-3.9))
print(as_int(\"7\"))
print(as_int(False))
print(sum([1, 2, 3]))
",
        },
        Case {
            name: "calling_a_function_passed_as_a_value",
            source: "\
def apply(fn, value):
    return fn(value)

def twice(value):
    return value * 2

def add(amount):
    def add_to(value):
        return value + amount

    return add_to

def reduce_all(values, fn, initial):
    accumulator = initial
    for value in values:
        accumulator = fn(accumulator, value)
    return accumulator

print(apply(twice, 21))
print(apply(lambda v: v + 1, 41))
print(apply(add(10), 5))
print(reduce_all([1, 2, 3, 4], lambda a, b: a * b, 1))
print(reduce_all([1, 2, 3, 4], lambda a, b: a + b, 0))
",
        },
        Case {
            name: "an_fstring_hole_prints_what_python_prints",
            source: "\
name = \"world\"
count = 3
ratio = 0.5
flag = True
nothing = None
items = [1, 2]

print(f\"hello {name}\")
print(f\"{count} and {count + 1}\")
print(f\"{name!r} {count!r}\")
print(f\"{{{count}}}\")
print(f\"{ratio:.2f} {count:03d}\")
print(f\"{flag} {nothing}\")
print(f\"nested {f\"inner {name}\"} done\")
print(f\"list {items}\")
print(f\"{name!s} {ratio!s}\")
print(f\"{name!r} {ratio!r}\")
",
        },
        Case {
            name: "a_specifier_needs_a_known_number",
            source: "\
def format_ratio(value: float) -> str:
    return f\"{value:.3f}\"

def format_count(value: int) -> str:
    return f\"[{value:04d}]\"

def pad(name: str) -> str:
    return f\"|{name:>6}|\"

print(format_ratio(2.5))
print(format_ratio(1))
print(format_count(7))
print(format_count(1234))
print(pad(\"ab\"))
",
        },
        Case {
            name: "untyped_parameters_compare_at_runtime",
            source: "\
def larger(a, b):
    if a > b:
        return a
    return b

print(larger(3, 7))
print(larger(\"a\", \"b\"))
print(larger(2.5, 1))

def same(a, b):
    return a == b

print(same(1, 1.0))
print(same([1], [1]))
print(same(\"x\", \"x\"))
",
        },
    ]
}

fn tools_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
        && Command::new("go")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
}

#[test]
fn emitted_go_computes_what_python_computes() {
    if !tools_available() {
        eprintln!("python3 or go is not on PATH; skipping the equivalence gate");
        return;
    }
    let dir = std::env::temp_dir().join(format!("gset-go-semantics-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");

    let mut failures: Vec<String> = Vec::new();
    for case in cases() {
        let expected = run_python(&dir, &case);
        let Some(expected) = expected else {
            failures.push(format!("{}: python3 did not run", case.name));
            continue;
        };

        let output = transpile(
            &format!("{}.py", case.name),
            case.source,
            Some("python"),
            "go",
        );
        let Ok(output) = output else {
            failures.push(format!("{}: the pipeline failed", case.name));
            continue;
        };
        if output.failed {
            failures.push(format!(
                "{}: the backend reported a gap: {}",
                case.name,
                output.diagnostics.render_one_line(&output.source_map)
            ));
            continue;
        }

        let program = dir.join(format!("{}.go", case.name));
        std::fs::write(&program, &output.text).expect("write Go");
        match run_go(&program) {
            Some(actual) if actual == expected => {}
            Some(actual) => failures.push(format!(
                "{}:\n  python: {expected:?}\n  go:     {actual:?}",
                case.name
            )),
            None => failures.push(format!(
                "{}: go run rejected the output:\n{}",
                case.name,
                run_go_verbose(&program)
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "the emitted Go computed something other than the source:\n\n{}",
        failures.join("\n\n")
    );
}

/// Reads the two runtimes' spellings of one value as equal.
///
/// Only the differences that are certainly rendering are collapsed: the two
/// spellings of a boolean, and a float whose value is whole. Both sides of a
/// comparison go through this, so a program that computed a different answer
/// still differs afterwards.
fn normalize(rendered: &str) -> String {
    rendered
        .split_whitespace()
        .map(|token| match token {
            "true" => "True".to_string(),
            "false" => "False".to_string(),
            "<nil>" => "None".to_string(),
            other => {
                let spelled_as_float = other.contains('.') && !other.contains('e');
                match other.parse::<f64>() {
                    Ok(value) if spelled_as_float && value.fract() == 0.0 => format!("{value:.0}"),
                    _ => other.to_string(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_python(dir: &Path, case: &Case) -> Option<String> {
    let source = dir.join(format!("{}.py", case.name));
    std::fs::write(&source, case.source).expect("write Python");
    let output = Command::new("python3")
        .arg(&source)
        .output()
        .expect("run python3");
    if !output.status.success() {
        eprintln!(
            "note: python itself rejected {}: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    Some(normalize(
        String::from_utf8_lossy(&output.stdout).trim_end(),
    ))
}

fn run_go(program: &std::path::Path) -> Option<String> {
    let output = Command::new("go")
        .arg("run")
        .arg(program)
        .output()
        .expect("run go");
    if !output.status.success() {
        return None;
    }
    Some(normalize(
        String::from_utf8_lossy(&output.stdout).trim_end(),
    ))
}

fn run_go_verbose(program: &std::path::Path) -> String {
    let output = Command::new("go")
        .arg("run")
        .arg(program)
        .output()
        .expect("run go");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
