package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"gsetlang/config"
	"gsetlang/lexer"
	"gsetlang/parser"
	"gsetlang/transpiler"
)

func main() {
	if len(os.Args) < 2 {
		printHelp()
		return
	}

	cfg := config.LoadConfig("")

	cmd := os.Args[1]

	switch cmd {
	case "run":
		if len(os.Args) < 3 {
			fmt.Println("Usage: gset run <file> [--keep]")
			return
		}
		keep := false
		if len(os.Args) > 3 && os.Args[3] == "--keep" {
			keep = true
		}
		runFile(os.Args[2], cfg, keep)
	case "transpile":
		if len(os.Args) < 3 {
			fmt.Println("Usage: gset transpile <file>")
			return
		}
		transpileFile(os.Args[2], cfg)
	case "version":
		fmt.Println("GSET v2.1.2")
	case "help":
		printHelp()
	default:
		if _, err := os.Stat(cmd); err == nil {
			runFile(cmd, cfg, false)
		} else {
			fmt.Printf("Unknown command: %s\n", cmd)
			printHelp()
		}
	}
}

func runFile(filename string, cfg config.GSETConfig, keep bool) {
	content, err := os.ReadFile(filename)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error reading file: %v\n", err)
		os.Exit(1)
	}

	ext := strings.TrimPrefix(filepath.Ext(filename), ".")
	if ext == filename {
		ext = "py"
	}

	l := lexer.New(string(content))
	p := parser.New(l)
	prog := p.ParseProgram()

	if len(p.Errors()) > 0 {
		fmt.Fprintf(os.Stderr, "Parse errors:\n")
		for _, e := range p.Errors() {
			fmt.Fprintf(os.Stderr, "  %s\n", e)
		}
		os.Exit(1)
	}

	exec := transpiler.NewExecutor(nil)
	exec.Execute(prog, ext, filename, keep)
}

func transpileFile(filename string, cfg config.GSETConfig) {
	content, err := os.ReadFile(filename)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error reading file: %v\n", err)
		os.Exit(1)
	}

	l := lexer.New(string(content))
	p := parser.New(l)
	prog := p.ParseProgram()

	if len(p.Errors()) > 0 {
		fmt.Fprintf(os.Stderr, "Parse errors:\n")
		for _, e := range p.Errors() {
			fmt.Fprintf(os.Stderr, "  %s\n", e)
		}
		os.Exit(1)
	}

	t := transpiler.New(cfg.GlobalKeywords)
	output := t.Translate(prog)
	fmt.Println(output)
}

func printHelp() {
	fmt.Println(`GSET - Generic Syntax Extension Tool v2.1.2

Usage:
  gset run <file>        Transpile and execute a file
  gset run <file> --keep Keep the execution file after running
  gset transpile <file>  Transpile and print output
  gset version           Show version
  gset help              Show this help

Supported features:
  - Variables: var, val, let, const
  - Functions: fn, func, def, async fn, lambda
  - Control Flow: if, elif, else, match, switch
  - Loops: for, foreach, while, do-while
  - Error Handling: try, catch, throw, finally
  - Classes: class, extends, implements, constructors
  - Async: async, await, yield
  - Type Annotations
  - List Comprehensions: [x for x in items if x > 0]
  - Pattern Matching: match, case, guard
  - Null Safety: ??, ?., optional chaining
  - Operators: &&, ||, ??, **, //, ===, !==
`)
}
