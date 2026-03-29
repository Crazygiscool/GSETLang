package parser

import (
	"testing"

	"gsetlang/lexer"
)

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
