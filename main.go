package main

import (
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"

	"gsetlang/config"
	"gsetlang/lexer"
	"gsetlang/logger"
	"gsetlang/parser"
	"gsetlang/security"
	"gsetlang/transpiler"
)

var version = "dev"

func main() {
	log := logger.Default()
	log.SetPrefix("main")

	if os.Getenv("GSET_DEBUG") != "" {
		logger.SetLevel(logger.DEBUG)
		log.Debug("debug mode enabled")
	}

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-sigChan
		log.Warn("received interrupt signal, shutting down...")
		os.Exit(130)
	}()

	if len(os.Args) < 2 {
		printHelp()
		return
	}

	cfg, err := config.LoadConfig("")
	if err != nil {
		log.Warn("failed to load config: %v", err)
	}

	if validation := cfg.Validate(); len(validation.Errors) > 0 {
		for _, err := range validation.Errors {
			log.Error("config validation error: %s", err)
		}
	}

	cmd := os.Args[1]

	switch cmd {
	case "run":
		if len(os.Args) < 3 {
			fmt.Println("Usage: gset run <file> [--target <lang>] [--keep]")
			return
		}
		keep, target, _ := parseFlags(os.Args[3:], true)
		runFile(os.Args[2], cfg, keep, target)
	case "transpile":
		if len(os.Args) < 3 {
			fmt.Println("Usage: gset transpile <file> [--target <lang>] [-o <outfile>]")
			return
		}
		_, target, outfile := parseFlags(os.Args[3:], false)
		transpileFile(os.Args[2], cfg, target, outfile)
	case "version":
		fmt.Println("GSET v" + version)
	case "help":
		printHelp()
	default:
		if _, err := os.Stat(cmd); err == nil {
			runFile(cmd, cfg, false, "")
		} else {
			fmt.Printf("Unknown command: %s\n", cmd)
			printHelp()
		}
	}
}

// parseFlags extracts the supported flags from a slice of arguments.
// When allowKeep is true, "--keep" is honored. Returns the keep flag, an
// optional target override, and an optional output path (transpile only).
func parseFlags(args []string, allowKeep bool) (keep bool, target, out string) {
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case allowKeep && arg == "--keep":
			keep = true
		case arg == "--target" || arg == "-t":
			if i+1 < len(args) {
				target = args[i+1]
				i++
			}
		case strings.HasPrefix(arg, "--target="):
			target = strings.TrimPrefix(arg, "--target=")
		case arg == "-o" || arg == "--output":
			if i+1 < len(args) {
				out = args[i+1]
				i++
			}
		case strings.HasPrefix(arg, "--output="):
			out = strings.TrimPrefix(arg, "--output=")
		}
	}
	return keep, target, out
}

func runFile(filename string, cfg config.GSETConfig, keep bool, target string) {
	log := logger.Default()

	if err := validateFilename(filename); err != nil {
		log.Error("invalid filename: %v", err)
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	log.Debug("reading file: %s", filename)

	content, err := os.ReadFile(filename)
	if err != nil {
		log.Error("failed to read file: %v", err)
		fmt.Fprintf(os.Stderr, "Error reading file: %v\n", err)
		os.Exit(1)
	}

	if err := security.ValidateInputSize(string(content)); err != nil {
		log.Error("input size validation failed: %v", err)
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	ext := strings.TrimPrefix(filepath.Ext(filename), ".")
	if ext == filename {
		ext = "py"
	}

	log.Debug("target extension: %s", ext)

	if !security.IsSafeFileExtension("." + ext) {
		log.Error("unsafe file extension: %s", ext)
		fmt.Fprintf(os.Stderr, "Error: unsafe file extension '%s'\n", ext)
		os.Exit(1)
	}

	if target != "" {
		n := transpiler.NormalizeTarget(target)
		if !transpiler.IsSupportedTarget(n) {
			fmt.Fprintf(os.Stderr, "Error: unsupported target '%s'\n", target)
			os.Exit(1)
		}
		target = n
	}

	log.Debug("lexing input")
	l := lexer.New(string(content))
	p := parser.New(l)
	prog := p.ParseProgram()

	if len(p.Errors()) > 0 {
		log.Error("parse errors: %v", p.Errors())
		fmt.Fprintf(os.Stderr, "Parse errors:\n")
		for _, e := range p.Errors() {
			fmt.Fprintf(os.Stderr, "  %s\n", e)
		}
		os.Exit(1)
	}

	log.Debug("executing transpiled code")
	exec := transpiler.NewExecutor(cfg)
	var execErr error
	if target != "" {
		execErr = exec.ExecuteFor(prog, target, filename, keep)
	} else {
		execErr = exec.Execute(prog, ext, filename, keep)
	}
	if execErr != nil {
		log.Error("execution failed: %v", execErr)
		fmt.Fprintf(os.Stderr, "Execution error: %v\n", execErr)
		os.Exit(1)
	}

	log.Info("completed successfully")
}

func transpileFile(filename string, cfg config.GSETConfig, target, outfile string) {
	log := logger.Default()

	if err := validateFilename(filename); err != nil {
		log.Error("invalid filename: %v", err)
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	log.Debug("reading file: %s", filename)

	content, err := os.ReadFile(filename)
	if err != nil {
		log.Error("failed to read file: %v", err)
		fmt.Fprintf(os.Stderr, "Error reading file: %v\n", err)
		os.Exit(1)
	}

	if err := security.ValidateInputSize(string(content)); err != nil {
		log.Error("input size validation failed: %v", err)
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	ext := strings.TrimPrefix(filepath.Ext(filename), ".")
	if ext == filename {
		ext = "py"
	}

	if target == "" {
		target = transpiler.ExtensionToTarget(ext)
	}
	target = transpiler.NormalizeTarget(target)
	if !transpiler.IsSupportedTarget(target) {
		fmt.Fprintf(os.Stderr, "Error: unsupported target '%s'\n", target)
		os.Exit(1)
	}

	log.Debug("lexing input")
	l := lexer.New(string(content))
	p := parser.New(l)
	prog := p.ParseProgram()

	if len(p.Errors()) > 0 {
		log.Error("parse errors: %v", p.Errors())
		fmt.Fprintf(os.Stderr, "Parse errors:\n")
		for _, e := range p.Errors() {
			fmt.Fprintf(os.Stderr, "  %s\n", e)
		}
		os.Exit(1)
	}

	log.Debug("transpiling to target: %s", target)
	t := transpiler.New(cfg.GetKeywords(nil, transpiler.TargetExtension(target)))
	output, err := t.TranslateFor(prog, target)
	if err != nil {
		log.Error("transpile failed: %v", err)
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	if outfile != "" {
		if err := os.WriteFile(outfile, []byte(output), 0644); err != nil {
			log.Error("failed to write output file: %v", err)
			fmt.Fprintf(os.Stderr, "Error writing output file: %v\n", err)
			os.Exit(1)
		}
	} else {
		fmt.Println(output)
	}

	log.Info("transpiled successfully")
}

func validateFilename(filename string) error {
	sanitized, err := security.SanitizeFilename(filename)
	if err != nil {
		return err
	}

	safePath, err := security.SanitizePath(filename)
	if err != nil {
		return err
	}

	_ = safePath
	_ = sanitized
	return nil
}

func printHelp() {
	fmt.Println("GSET - Generic Syntax Extension Tool v" + version + `

Usage:
  gset run <file>                        Transpile and execute a file
  gset run <file> --target <lang>        Run for a specific target
  gset run <file> --keep                 Keep the execution file after running
  gset transpile <file>                  Transpile and print output
  gset transpile <file> --target <lang>  Transpile to a specific target
  gset transpile <file> -o <outfile>     Write output to a file
  gset version                           Show version
  gset help                              Show this help

Targets:
  go, python, javascript, java, ruby

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
