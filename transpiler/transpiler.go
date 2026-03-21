package transpiler

import (
	"fmt"
	"gsetlang/ast"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

type Transpiler struct {
	cfg map[string]string
}

func New(cfg map[string]string) *Transpiler {
	return &Transpiler{cfg: cfg}
}

func (t *Transpiler) Translate(program *ast.Program) string {
	var out string
	for _, stmt := range program.Statements {
		translated := t.translateStatement(stmt)
		if translated != "" {
			out += translated + "\n"
		}
	}
	return strings.TrimSpace(out)
}

func (t *Transpiler) translateStatement(stmt ast.Statement) string {
	switch s := stmt.(type) {
	case *ast.ExpressionStatement:
		return t.translateExpression(s.Expression)
	case *ast.VariableStatement:
		return "var " + s.Name + " = " + t.translateExpression(s.Value)
	case *ast.AssignmentStatement:
		return s.Name + " " + s.Operator + " " + t.translateExpression(s.Value)
	case *ast.IfStatement:
		return t.translateIfStatement(s)
	case *ast.ForStatement:
		return t.translateForStatement(s)
	case *ast.WhileStatement:
		return t.translateWhileStatement(s)
	case *ast.FunctionStatement:
		return t.translateFunctionStatement(s)
	case *ast.ReturnStatement:
		if s.Value != nil {
			return "return " + t.translateExpression(s.Value)
		}
		return "return"
	case *ast.BreakStatement:
		return "break"
	case *ast.ContinueStatement:
		return "continue"
	}
	return ""
}

func (t *Transpiler) translateIfStatement(s *ast.IfStatement) string {
	out := "if " + t.translateExpression(s.Condition) + " {\n"
	for _, stmt := range s.Consequence.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	if s.Alternative != nil {
		out += " else {\n"
		for _, stmt := range s.Alternative.Statements {
			out += "    " + t.translateStatement(stmt) + "\n"
		}
		out += "}"
	}
	return out
}

func (t *Transpiler) translateForStatement(s *ast.ForStatement) string {
	init := t.translateStatement(s.Init)
	cond := t.translateExpression(s.Condition)
	update := t.translateStatement(s.Update)
	out := "for " + init + "; " + cond + "; " + update + " {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	return out
}

func (t *Transpiler) translateWhileStatement(s *ast.WhileStatement) string {
	out := "while " + t.translateExpression(s.Condition) + " {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	return out
}

func (t *Transpiler) translateFunctionStatement(s *ast.FunctionStatement) string {
	out := "func " + s.Name + "(" + strings.Join(s.Parameters, ", ") + ") {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	return out
}

func (t *Transpiler) translateExpression(expr ast.Expression) string {
	switch e := expr.(type) {
	case *ast.CallExpression:
		return t.translateCallExpression(e)
	case *ast.Identifier:
		return e.Value
	case *ast.IntegerLiteral:
		return strconv.FormatInt(e.Value, 10)
	case *ast.FloatLiteral:
		return strconv.FormatFloat(e.Value, 'f', -1, 64)
	case *ast.StringLiteral:
		return fmt.Sprintf(`"%s"`, e.Value)
	case *ast.BooleanLiteral:
		if e.Value {
			return "true"
		}
		return "false"
	case *ast.ArrayLiteral:
		var els []string
		for _, el := range e.Elements {
			els = append(els, t.translateExpression(el))
		}
		return "[]interface{}{" + strings.Join(els, ", ") + "}"
	case *ast.MapLiteral:
		return "map[string]interface{}{}"
	case *ast.PrefixExpression:
		return e.Operator + t.translateExpression(e.Right)
	case *ast.InfixExpression:
		return t.translateInfixExpression(e)
	case *ast.IndexExpression:
		return t.translateExpression(e.Left) + "[" + t.translateExpression(e.Index) + "]"
	}
	return ""
}

func (t *Transpiler) translateCallExpression(ce *ast.CallExpression) string {
	fn := ce.Function
	if mapping, ok := t.cfg[fn]; ok {
		fn = mapping
	}
	var args []string
	for _, arg := range ce.Arguments {
		args = append(args, t.translateExpression(arg))
	}
	return fn + "(" + strings.Join(args, ", ") + ")"
}

func (t *Transpiler) translateInfixExpression(ie *ast.InfixExpression) string {
	left := t.translateExpression(ie.Left)
	right := t.translateExpression(ie.Right)

	if ie.Operator == "&&" {
		left = "(" + left + ")"
		right = "(" + right + ")"
	}

	return left + " " + ie.Operator + " " + right
}

type Executor struct {
	cfg       map[string]string
	compilers map[string]CompilerConfig
}

type CompilerConfig struct {
	Command string
	Args    string
	Wrapper string
	Run     string
}

func NewExecutor(cfg map[string]string, compilers map[string]CompilerConfig) *Executor {
	return &Executor{cfg: cfg, compilers: compilers}
}

func (e *Executor) Execute(code, ext, filename string) {
	needsFmt := strings.Contains(code, "fmt.")

	wrapper := ""
	if ext == "go" {
		wrapper = "package main\n"
		if needsFmt {
			wrapper += "import \"fmt\"\n"
		}
		wrapper += "func main() {\n    " + code + "\n}"
	} else {
		wrapper = code
	}

	var tmpFile string
	switch ext {
	case "py":
		tmpFile = "/tmp/gset_" + filename + ".py"
	case "js":
		tmpFile = "/tmp/gset_" + filename + ".js"
	case "java":
		tmpFile = "Main.java"
	case "rb":
		tmpFile = "/tmp/gset_" + filename + ".rb"
	case "php":
		tmpFile = "/tmp/gset_" + filename + ".php"
	case "rs":
		tmpFile = "/tmp/gset_" + filename + ".rs"
	case "swift":
		tmpFile = "/tmp/gset_" + filename + ".swift"
	case "kt":
		tmpFile = "/tmp/gset_" + filename + ".kt"
	case "c":
		tmpFile = "/tmp/gset_" + filename + ".c"
	case "cpp":
		tmpFile = "/tmp/gset_" + filename + ".cpp"
	case "cs":
		tmpFile = "/tmp/gset_" + filename + ".cs"
	case "go":
		tmpFile = "/tmp/gset_" + filename + ".go"
	default:
		tmpFile = "/tmp/gset_exec." + ext
	}

	compCfg, ok := e.compilers[ext]
	if !ok {
		compCfg = e.compilers["go"]
		ext = "go"
	}

	if compCfg.Wrapper != "" && ext != "go" {
		wrapper = strings.Replace(compCfg.Wrapper, "##CODE##", code, -1)
	}

	os.WriteFile(tmpFile, []byte(wrapper), 0644)

	fmt.Printf("--- COMPILING WITH %s ---\n", ext)

	if compCfg.Run != "" {
		cmd := exec.Command("sh", strings.Fields(compCfg.Args)...)
		cmd.Args = append(cmd.Args, compCfg.Run)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		fmt.Println("--- RUNNING GSET OUTPUT ---")
		cmd.Run()
		return
	}

	var args []string
	if compCfg.Args != "" {
		args = strings.Split(compCfg.Args, " ")
	}
	args = append(args, tmpFile)
	cmd := exec.Command(compCfg.Command, args...)

	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	fmt.Println("--- RUNNING GSET OUTPUT ---")
	cmd.Run()

	if ext == "java" {
		os.Remove("Main.class")
	}
}
