package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"gsetlang/ast"
	"gsetlang/config"
	"gsetlang/lexer"
	"gsetlang/parser"
	"gsetlang/transpiler"
)

const Version = "2.0.2"

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	cmd := os.Args[1]

	switch cmd {
	case "-v", "--version", "version", "v":
		fmt.Println("GSET version", Version)
	case "-h", "--help", "help":
		printUsage()
	case "-i", "--install":
		install()
	default:
		runFile(cmd)
	}
}

func printUsage() {
	fmt.Println("GSET - Generic Syntax Extension Tool")
	fmt.Println("Version:", Version)
	fmt.Println("")
	fmt.Println("Usage:")
	fmt.Println("  gset <file>          Run a GSET file")
	fmt.Println("  gset version        Show version")
	fmt.Println("  gset install        Install GSET to system")
	fmt.Println("  gset help           Show this help")
}

func install() {
	// Basic install - copy binary to /usr/local/bin
	exePath, _ := os.Executable()
	dest := "/usr/local/bin/gset"

	fmt.Printf("Installing GSET %s to %s...\n", Version, dest)
	fmt.Println("Note: This requires sudo/root privileges")

	// Try to copy (will fail without sudo - that's OK for now)
	fmt.Println("To install manually: sudo cp", exePath, dest)
}

func runFile(filePath string) {
	ext := strings.TrimPrefix(filepath.Ext(filePath), ".")
	filename := strings.TrimSuffix(filepath.Base(filePath), filepath.Ext(filePath))

	// Default to Go if no extension or unknown extension
	if ext == "" || ext == "gset" {
		ext = "go"
	}

	content, err := os.ReadFile(filePath)
	if err != nil {
		fmt.Println("Error reading file:", err)
		os.Exit(1)
	}

	fileKeywords, body := parseGSetFile(string(content))

	globalConfig := config.LoadConfig(filePath)
	mergedKeywords := globalConfig.GetKeywords(fileKeywords, ext)

	l := lexer.New(body)
	p := parser.New(l)

	program := p.ParseProgram()

	if p.Errors() != nil {
		fmt.Println("Parser errors:", p.Errors())
	}

	fmt.Println("--- AST ---")
	fmt.Printf("Statements: %d\n", len(program.Statements))
	fmt.Println(programToString(program))

	fmt.Println("--- TRANSLATED ---")
	t := transpiler.New(mergedKeywords)
	translated := t.Translate(program)
	fmt.Println(translated)

	exec := transpiler.NewExecutor(mergedKeywords, convertCompilers(globalConfig.Compilers))
	exec.Execute(translated, ext, filename)
}

func convertCompilers(src map[string]config.CompilerConfig) map[string]transpiler.CompilerConfig {
	dst := make(map[string]transpiler.CompilerConfig)
	for k, v := range src {
		dst[k] = transpiler.CompilerConfig{
			Command: v.Command,
			Args:    v.Args,
			Wrapper: v.Wrapper,
			Run:     v.Run,
		}
	}
	return dst
}

func parseGSetFile(src string) (map[string]string, string) {
	keywords := make(map[string]string)

	parts := strings.SplitN(src, "---", 2)
	if len(parts) < 2 {
		return keywords, src
	}

	header := parts[0]
	lines := strings.Split(header, "\n")

	for _, line := range lines {
		pair := strings.SplitN(line, "=", 2)
		if len(pair) == 2 {
			key := strings.TrimSpace(pair[0])
			val := strings.TrimSpace(pair[1])
			if key != "" {
				keywords[key] = val
			}
		}
	}

	return keywords, parts[1]
}

func programToString(program *ast.Program) string {
	var out string
	for _, stmt := range program.Statements {
		out += stmt.String() + "\n"
	}
	return out
}
