package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestIntegration_HelloWorld(t *testing.T) {
	content := `print("Hello, World!")`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}

	if !strings.Contains(string(output), "Hello, World!") {
		t.Errorf("expected output to contain 'Hello, World!', got: %s", output)
	}
}

func TestIntegration_Variables(t *testing.T) {
	content := `
x = 5
print(x)
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_ForLoop(t *testing.T) {
	content := `
nums = [1, 2, 3]
for i in nums {
    print(i)
}
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_WhileLoop(t *testing.T) {
	content := `
count = 3
while count > 0 {
    print(count)
    count = count - 1
}
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_IfElse(t *testing.T) {
	content := `
x = 10
if x > 5 {
    print("big")
} else {
    print("small")
}
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}

	if !strings.Contains(string(output), "big") {
		t.Errorf("expected output to contain 'big', got: %s", output)
	}
}

func TestIntegration_Function(t *testing.T) {
	content := `
function add(a, b) {
    return a + b
}

result = add(2, 3)
print(result)
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_Transpile(t *testing.T) {
	content := `print("test")`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "transpile", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}

	if !strings.Contains(string(output), "fmt.Println") {
		t.Errorf("expected transpiled output to contain 'fmt.Println', got: %s", output)
	}
}

func TestIntegration_TryCatch(t *testing.T) {
	content := `
try {
    print("try")
} catch e {
    print("catch")
} finally {
    print("finally")
}
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_Class(t *testing.T) {
	content := `
class Person {
    name = "John"
    function greet() {
        print("Hello")
    }
}
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_ListComprehension(t *testing.T) {
	content := `
nums = [1, 2, 3, 4, 5]
squared = [x * x for x in nums]
print(squared)
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_Match(t *testing.T) {
	content := `
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

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}

func TestIntegration_TranspileToPython(t *testing.T) {
	content := `print("hello")`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.py")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "transpile", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}

	if !strings.Contains(string(output), "print") {
		t.Errorf("expected transpiled output to contain 'print', got: %s", output)
	}
}

func TestIntegration_ErrorHandling(t *testing.T) {
	content := `
function broken() {
    this is invalid syntax
}
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err == nil {
		t.Errorf("expected command to fail for invalid syntax, got output: %s", output)
	}
}

func TestIntegration_NestedFunctions(t *testing.T) {
	content := `
function outer() {
    x = 10
    function inner() {
        return x + 1
    }
    return inner()
}
print(outer())
`

	tmpDir := t.TempDir()
	testFile := filepath.Join(tmpDir, "test.gset")

	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	cmd := exec.Command("./gset", "run", testFile)
	output, err := cmd.CombinedOutput()

	if err != nil {
		t.Fatalf("command failed: %v, output: %s", err, output)
	}
}
