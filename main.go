package main

import (
	"fmt"
	"os"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Println("Usage: gset <filename>")
		os.Exit(1)
	}

	testInput := fileparse(os.Args[1])
	config, body := ParseGSet(testInput)

	l := NewLexer(body)
	p := NewParser(l)

	fmt.Println("--- AST ---")
	fmt.Println(p.String())

	fmt.Println("--- TRANSLATED ---")
	t := NewTranspiler(config)
	translated := t.Translate(p.program)
	fmt.Println(translated)

	Execute(translated)
}

func fileparse(filepath string) string {
	content, err := os.ReadFile(filepath)

	if err != nil {
		fmt.Println("Error reading file:", err)
		return ""
	}

	return string(content)
}
