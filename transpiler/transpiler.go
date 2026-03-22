package transpiler

import (
	"fmt"
	"gsetlang/ast"
	"os"
	"os/exec"
	"path/filepath"
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
		name := ""
		if s.Name != nil {
			name = s.Name.Value
		}
		return "var " + name + " = " + t.translateExpression(s.Value)
	case *ast.AssignmentStatement:
		return t.translateExpression(s.Name) + " " + s.Operator + " " + t.translateExpression(s.Value)
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
		if s.Label != "" {
			return "break " + s.Label
		}
		return "break"
	case *ast.ContinueStatement:
		if s.Label != "" {
			return "continue " + s.Label
		}
		return "continue"
	case *ast.ThrowStatement:
		return "throw " + t.translateExpression(s.Value)
	case *ast.TryStatement:
		return t.translateTryStatement(s)
	case *ast.MatchStatement:
		return t.translateMatchStatement(s)
	case *ast.ForClassicStatement:
		return t.translateForClassicStatement(s)
	case *ast.ForEachStatement:
		return t.translateForEachStatement(s)
	case *ast.DoWhileStatement:
		return t.translateDoWhileStatement(s)
	case *ast.ClassStatement:
		return t.translateClassStatement(s)
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
		if altIf, ok := s.Alternative.(*ast.IfStatement); ok {
			out += " else " + t.translateIfStatement(altIf)
		} else if altBlock, ok := s.Alternative.(*ast.BlockStatement); ok {
			out += " else {\n"
			for _, stmt := range altBlock.Statements {
				out += "    " + t.translateStatement(stmt) + "\n"
			}
			out += "}"
		}
	}
	return out
}

func (t *Transpiler) translateForStatement(s *ast.ForStatement) string {
	item := "item"
	if s.Item != nil {
		item = s.Item.Value
	}
	out := "for " + item + " in " + t.translateExpression(s.Iterable) + " {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	return out
}

func (t *Transpiler) translateForClassicStatement(s *ast.ForClassicStatement) string {
	init := ""
	if s.Init != nil {
		init = t.translateStatement(s.Init)
	}
	cond := ""
	if s.Condition != nil {
		cond = t.translateExpression(s.Condition)
	}
	update := ""
	if s.Update != nil {
		update = t.translateStatement(s.Update)
	}
	out := "for " + init + "; " + cond + "; " + update + " {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	return out
}

func (t *Transpiler) translateForEachStatement(s *ast.ForEachStatement) string {
	key := "_"
	value := "item"
	if s.Key != nil {
		key = s.Key.Value
	}
	if s.Value != nil {
		value = s.Value.Value
	}
	out := "for " + key + ", " + value + " in " + t.translateExpression(s.Object) + " {\n"
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

func (t *Transpiler) translateDoWhileStatement(s *ast.DoWhileStatement) string {
	out := "do {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "} while (" + t.translateExpression(s.Condition) + ")"
	return out
}

func (t *Transpiler) translateForStatementOld(s *ast.ForStatement) string {
	return t.translateForStatement(s)
}

func (t *Transpiler) translateFunctionStatement(s *ast.FunctionStatement) string {
	name := ""
	if s.Name != nil {
		name = s.Name.Value
	}
	var params []string
	for _, p := range s.Parameters {
		params = append(params, p.Name.Value)
	}
	out := "func " + name + "(" + strings.Join(params, ", ") + ") {\n"
	for _, stmt := range s.Body.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	return out
}

func (t *Transpiler) translateTryStatement(s *ast.TryStatement) string {
	out := "try {\n"
	for _, stmt := range s.TryBlock.Statements {
		out += "    " + t.translateStatement(stmt) + "\n"
	}
	out += "}"
	for _, c := range s.Catches {
		varName := ""
		if c.Variable != nil {
			varName = c.Variable.Value
		}
		out += " catch (" + varName + ") {\n"
		for _, stmt := range c.Body.Statements {
			out += "    " + t.translateStatement(stmt) + "\n"
		}
		out += "}"
	}
	if s.Finally != nil {
		out += " finally {\n"
		for _, stmt := range s.Finally.Statements {
			out += "    " + t.translateStatement(stmt) + "\n"
		}
		out += "}"
	}
	return out
}

func (t *Transpiler) translateMatchStatement(s *ast.MatchStatement) string {
	out := "switch " + t.translateExpression(s.Subject) + " {\n"
	for _, c := range s.Cases {
		out += "case " + t.translateExpression(c.Pattern) + ":\n"
		for _, stmt := range c.Consequence.Statements {
			out += "    " + t.translateStatement(stmt) + "\n"
		}
	}
	out += "}"
	return out
}

func (t *Transpiler) translateClassStatement(s *ast.ClassStatement) string {
	name := ""
	if s.Name != nil {
		name = s.Name.Value
	}
	ext := ""
	if s.Extends != nil {
		ext = " extends " + s.Extends.Value
	}
	out := "class " + name + ext + " {\n"
	for _, p := range s.Properties {
		if p.Name != nil {
			out += "    " + p.Name.Value
			if p.Value != nil {
				out += " = " + t.translateExpression(p.Value)
			}
			out += "\n"
		}
	}
	for _, m := range s.Methods {
		out += "    " + t.translateFunctionStatement(m) + "\n"
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
	case *ast.NilLiteral:
		return "nil"
	case *ast.ArrayLiteral:
		var els []string
		for _, el := range e.Elements {
			els = append(els, t.translateExpression(el))
		}
		return "[]interface{}{" + strings.Join(els, ", ") + "}"
	case *ast.MapLiteral:
		var pairs []string
		for _, p := range e.Pairs {
			pairs = append(pairs, t.translateExpression(p.Key)+": "+t.translateExpression(p.Value))
		}
		return "map[string]interface{}{" + strings.Join(pairs, ", ") + "}"
	case *ast.PrefixExpression:
		return e.Operator + t.translateExpression(e.Right)
	case *ast.InfixExpression:
		return t.translateInfixExpression(e)
	case *ast.IndexExpression:
		return t.translateExpression(e.Left) + "[" + t.translateExpression(e.Index) + "]"
	case *ast.MemberExpression:
		return t.translateExpression(e.Object) + "." + e.Property.Value
	case *ast.MethodCallExpression:
		args := make([]string, len(e.Arguments))
		for i, a := range e.Arguments {
			args[i] = t.translateExpression(a)
		}
		return t.translateExpression(e.Object) + "." + e.Method.Value + "(" + strings.Join(args, ", ") + ")"
	case *ast.TernaryExpression:
		return t.translateExpression(e.Condition) + " ? " + t.translateExpression(e.Consequence) + " : " + t.translateExpression(e.Alternative)
	case *ast.NullCoalescingExpression:
		return t.translateExpression(e.Left) + " ?? " + t.translateExpression(e.Right)
	case *ast.LambdaExpression:
		return t.translateLambda(e)
	case *ast.ListComprehension:
		return t.translateListComprehension(e)
	case *ast.AwaitExpression:
		return "await " + t.translateExpression(e.Value)
	case *ast.YieldExpression:
		if e.Value != nil {
			return "yield " + t.translateExpression(e.Value)
		}
		return "yield"
	case *ast.TypeExpression:
		return e.Name
	}
	return ""
}

func (t *Transpiler) translateCallExpression(ce *ast.CallExpression) string {
	fn := ""
	if ident, ok := ce.Function.(*ast.Identifier); ok {
		fn = ident.Value
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

func (t *Transpiler) translateLambda(e *ast.LambdaExpression) string {
	var params []string
	for _, p := range e.Parameters {
		params = append(params, p.Name.Value)
	}
	if e.Expression != nil {
		return "fn(" + strings.Join(params, ", ") + ") => " + t.translateExpression(e.Expression)
	}
	body := ""
	for _, stmt := range e.Body.Statements {
		body += t.translateStatement(stmt) + "\n"
	}
	return "fn(" + strings.Join(params, ", ") + ") {\n    " + body + "}"
}

func (t *Transpiler) translateListComprehension(e *ast.ListComprehension) string {
	element := t.translateExpression(e.Element)
	item := "item"
	if e.Variable != nil {
		item = e.Variable.Value
	}
	iterable := t.translateExpression(e.Iterable)
	if e.Condition != nil {
		return "[" + element + " for " + item + " in " + iterable + " if " + t.translateExpression(e.Condition) + "]"
	}
	return "[" + element + " for " + item + " in " + iterable + "]"
}

type Executor struct {
	compilers map[string]CompilerConfig
}

type CompilerConfig struct {
	Command string
	Args    string
	Wrapper string
	Run     string
}

func NewExecutor(compilers map[string]CompilerConfig) *Executor {
	if compilers == nil {
		compilers = GetCompilers()
	}
	return &Executor{compilers: compilers}
}

func GetCompilers() map[string]CompilerConfig {
	return map[string]CompilerConfig{
		"py":   {Command: "python3", Args: "", Wrapper: ""},
		"js":   {Command: "node", Args: "", Wrapper: ""},
		"go":   {Command: "go run", Args: "", Wrapper: ""},
		"rb":   {Command: "ruby", Args: "", Wrapper: ""},
		"java": {Command: "java", Args: "", Wrapper: "public class Main { public static void main(String[] args) { ##CODE## }}"},
	}
}

func (e *Executor) Execute(program *ast.Program, ext, filename string, keep bool) {
	t := New(nil)
	code := t.Translate(program)

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

	compCfg, ok := e.compilers[ext]
	if !ok {
		compCfg = e.compilers["go"]
		ext = "go"
	}

	if compCfg.Wrapper != "" && ext != "go" {
		wrapper = strings.Replace(compCfg.Wrapper, "##CODE##", code, -1)
	}

	var tmpFile string
	baseName := filepath.Base(filename)
	switch ext {
	case "py":
		tmpFile = "/tmp/gset_" + baseName + ".py"
	case "js":
		tmpFile = "/tmp/gset_" + filename + ".js"
	case "java":
		tmpFile = "Main.java"
	case "rb":
		tmpFile = "/tmp/gset_" + filename + ".rb"
	case "go":
		tmpFile = "/tmp/gset_" + filename + ".go"
	default:
		tmpFile = "/tmp/gset_exec." + ext
	}

	os.WriteFile(tmpFile, []byte(wrapper), 0644)

	fmt.Printf("--- COMPILING WITH %s ---\n", ext)

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

	if !keep {
		os.Remove(tmpFile)
		if ext == "java" {
			os.Remove("Main.class")
		}
	}
}
