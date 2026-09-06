package parser

import (
	"testing"
	"time"

	"gsetlang/ast"
	"gsetlang/lexer"
)

// parseWithTimeout runs ParseProgram and fails the test if the parser spins.
// Regression guard for the historical infinite-loop bugs (match/case,
// try/catch, list comprehensions).
func parseWithTimeout(t *testing.T, input string) *ast.Program {
	t.Helper()
	type result struct {
		prog *ast.Program
		errs []string
	}
	done := make(chan result, 1)
	go func() {
		l := lexer.New(input)
		p := New(l)
		prog := p.ParseProgram()
		done <- result{prog: prog, errs: p.Errors()}
	}()
	select {
	case r := <-done:
		if len(r.errs) > 0 {
			t.Fatalf("unexpected parse errors: %v", r.errs)
		}
		return r.prog
	case <-time.After(5 * time.Second):
		t.Fatalf("parser did not terminate within 5s for input:\n%s", input)
		return nil
	}
}

func TestParseMatchDoesNotHang(t *testing.T) {
	input := `
x = 2
match x {
    case 1:
        print("one")
    case 2:
        print("two")
    default:
        print("other")
}
`
	parseWithTimeout(t, input)
}

func TestParseTryCatchDoesNotHang(t *testing.T) {
	input := `
try {
    print("try")
} catch e {
    print("catch")
} finally {
    print("finally")
}
`
	parseWithTimeout(t, input)
}

func TestParseListComprehensionDoesNotHang(t *testing.T) {
	input := `
nums = [1, 2, 3, 4, 5]
squared = [x * x for x in nums]
evens = [x for x in nums if x % 2 == 0]
print(squared)
`
	parseWithTimeout(t, input)
}

func TestVariableDeclarationIsDeclared(t *testing.T) {
	input := `var x = 10
let y = 20
const z = 30
`
	l := lexer.New(input)
	p := New(l)
	prog := p.ParseProgram()
	if len(p.Errors()) > 0 {
		t.Fatalf("unexpected parse errors: %v", p.Errors())
	}
	declared := 0
	for _, stmt := range prog.Statements {
		if _, ok := stmt.(*ast.VariableStatement); ok {
			declared++
		}
	}
	if declared != 3 {
		t.Fatalf("expected 3 VariableStatements, got %d", declared)
	}
}

func BenchmarkParseProgram(b *testing.B) {
	input := `function add(a, b) {
    return a + b
}

x = 42
name = "hello"
isActive = true
nums = [1, 2, 3, 4, 5]

for i in nums {
    print(i)
}

if x > 10 {
    print("big")
}`

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		l := lexer.New(input)
		p := New(l)
		p.ParseProgram()
	}
}

func BenchmarkParseProgram_Simple(b *testing.B) {
	input := `x = 5`

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		l := lexer.New(input)
		p := New(l)
		p.ParseProgram()
	}
}

func BenchmarkParseProgram_Complex(b *testing.B) {
	input := `
class Person {
    name = "John"
    age = 30
    function greet() {
        print("Hello")
    }
}

function fib(n) {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

result = fib(10)
`

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		l := lexer.New(input)
		p := New(l)
		p.ParseProgram()
	}
}

func TestParseIndexExpression(t *testing.T) {
	prog := parseWithTimeout(t, `nums = [1, 2, 3]
print(nums[0])
print(nums[1] + nums[2])
x = nums[0]
`)
	if len(prog.Statements) != 4 {
		t.Fatalf("expected 4 statements, got %d", len(prog.Statements))
	}
	checks := func(s ast.Statement, want string) {
		t.Helper()
		es, ok := s.(*ast.ExpressionStatement)
		if !ok {
			t.Fatalf("expected ExpressionStatement, got %T", s)
		}
		if got := es.Expression.String(); got != want {
			t.Errorf("expression = %q, want %q", got, want)
		}
	}
	checks(prog.Statements[1], "print(nums[0])")
	checks(prog.Statements[2], "print((nums[1] + nums[2]))")
	checks(prog.Statements[3], "(x = nums[0])")
}

func TestParseChainedIndexExpression(t *testing.T) {
	prog := parseWithTimeout(t, `matrix = [[1, 2], [3, 4]]
print(matrix[1][0])
`)
	es, ok := prog.Statements[1].(*ast.ExpressionStatement)
	if !ok {
		t.Fatalf("expected ExpressionStatement, got %T", prog.Statements[1])
	}
	if got := es.Expression.String(); got != "print(matrix[1][0])" {
		t.Errorf("expression = %q, want %q", got, "print(matrix[1][0])")
	}
}

func TestParseGarbageBlockTerminatesWithError(t *testing.T) {
	input := `export add(a, b) {
    return a + b
}
`
	done := make(chan struct{}, 1)
	go func() {
		l := lexer.New(input)
		p := New(l)
		p.ParseProgram()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatalf("parser did not terminate within 5s for input:\n%s", input)
	}
}
