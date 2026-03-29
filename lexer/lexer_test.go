package lexer

import (
	"testing"
)

func BenchmarkNextToken(b *testing.B) {
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
		l := New(input)
		for {
			tok := l.NextToken()
			if tok.Type == "EOF" {
				break
			}
		}
	}
}

func BenchmarkNextToken_Simple(b *testing.B) {
	input := `x = 5`

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		l := New(input)
		for {
			tok := l.NextToken()
			if tok.Type == "EOF" {
				break
			}
		}
	}
}

func BenchmarkNextToken_Complex(b *testing.B) {
	input := `
class Person {
    name = "John"
    age = 30
    function greet() {
        print("Hello, " + name)
    }
    function getAge() {
        return age
    }
}

obj = new Person()
obj.greet()
result = obj.getAge()
`

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		l := New(input)
		for {
			tok := l.NextToken()
			if tok.Type == "EOF" {
				break
			}
		}
	}
}
