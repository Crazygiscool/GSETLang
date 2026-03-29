package transpiler

import (
	"testing"

	"gsetlang/lexer"
	"gsetlang/parser"
)

func BenchmarkTranslate(b *testing.B) {
	input := `function add(a, b) {
    return a + b
}

x = 42
name = "hello"
nums = [1, 2, 3]

for i in nums {
    print(i)
}

if x > 10 {
    print("big")
}`

	l := lexer.New(input)
	p := parser.New(l)
	prog := p.ParseProgram()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		tk := New(nil)
		tk.Translate(prog)
	}
}

func BenchmarkTranslate_Simple(b *testing.B) {
	input := `x = 5`

	l := lexer.New(input)
	p := parser.New(l)
	prog := p.ParseProgram()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		tk := New(nil)
		tk.Translate(prog)
	}
}

func BenchmarkTranslate_Complex(b *testing.B) {
	input := `
class Calculator {
    value = 0
    
    function add(n) {
        value = value + n
        return value
    }
    
    function subtract(n) {
        value = value - n
        return value
    }
    
    function multiply(n) {
        value = value * n
        return value
    }
}

calc = new Calculator()
calc.add(10)
calc.subtract(3)
result = calc.multiply(2)
`

	l := lexer.New(input)
	p := parser.New(l)
	prog := p.ParseProgram()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		tk := New(nil)
		tk.Translate(prog)
	}
}
