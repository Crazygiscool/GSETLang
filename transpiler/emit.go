package transpiler

import (
	"fmt"
	"strings"

	"gsetlang/ast"
)

// NormalizeTarget maps a user-supplied target name (or file extension) to a
// canonical backend identifier. Unrecognized names are returned unchanged so
// callers can produce a "target not supported" error.
func NormalizeTarget(s string) string {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "go", "golang":
		return "go"
	case "python", "py":
		return "python"
	case "javascript", "js", "node", "nodejs":
		return "javascript"
	case "java":
		return "java"
	case "ruby", "rb":
		return "ruby"
	}
	return s
}

// ExtensionToTarget maps a source file extension to a canonical target.
// Unknown or GSET-native extensions fall back to the "go" target, which is
// the default intermediate representation.
func ExtensionToTarget(ext string) string {
	switch strings.ToLower(strings.TrimPrefix(ext, ".")) {
	case "py":
		return "python"
	case "js", "ts":
		return "javascript"
	case "go":
		return "go"
	case "java":
		return "java"
	case "rb":
		return "ruby"
	}
	if IsSupportedTarget(ext) {
		return NormalizeTarget(ext)
	}
	return "go"
}

const indentUnit = "    "

// emitter renders a parsed program into one target language. Each target is
// emitted as a complete, runnable source file.
type emitter struct {
	target string
	kw     map[string]string
}

// supportedTargets lists backends with a working code generator.
var supportedTargets = []string{"go", "python", "javascript", "java", "ruby"}

func (e *emitter) ind(n int) string { return strings.Repeat(indentUnit, n) }

func (e *emitter) emitProgram(prog *ast.Program) string {
	switch e.target {
	case "go":
		return e.emitGo(prog)
	case "python":
		return e.emitPython(prog)
	case "javascript":
		return e.emitJavaScript(prog)
	case "java":
		return e.emitJava(prog)
	case "ruby":
		return e.emitRuby(prog)
	}
	return ""
}

// ---------------------------------------------------------------------------
// Shared expression helpers
// ---------------------------------------------------------------------------

func (e *emitter) expr(x ast.Expression) string {
	switch x.(type) {
	case nil:
		return "null"
	}
	switch v := x.(type) {
	case *ast.Identifier:
		return v.Value
	case *ast.IntegerLiteral:
		return fmt.Sprintf("%d", v.Value)
	case *ast.FloatLiteral:
		return fmt.Sprintf("%g", v.Value)
	case *ast.StringLiteral:
		return fmt.Sprintf("%q", v.Value)
	case *ast.BooleanLiteral:
		if v.Value {
			return e.boolLit(true)
		}
		return e.boolLit(false)
	case *ast.NilLiteral:
		return e.nilLit()
	case *ast.ArrayLiteral:
		return e.arrayLit(v)
	case *ast.MapLiteral:
		return e.mapLit(v)
	case *ast.ListComprehension:
		return e.comprehension(v)
	case *ast.CallExpression:
		return e.callExpr(v)
	case *ast.MethodCallExpression:
		return e.expr(v.Object) + "." + v.Method.Value + "(" + e.args(v.Arguments) + ")"
	case *ast.MemberExpression:
		if v.Computed {
			return e.expr(v.Object) + "[" + v.Property.Value + "]"
		}
		return e.expr(v.Object) + "." + v.Property.Value
	case *ast.IndexExpression:
		return e.expr(v.Left) + "[" + e.expr(v.Index) + "]"
	case *ast.PrefixExpression:
		return e.prefix(v)
	case *ast.InfixExpression:
		return e.infix(v)
	case *ast.TernaryExpression:
		return e.ternary(v)
	case *ast.NullCoalescingExpression:
		return e.coalesce(v)
	case *ast.LambdaExpression:
		return e.lambda(v)
	case *ast.AwaitExpression:
		return "await " + e.expr(v.Value)
	case *ast.YieldExpression:
		if v.Value != nil {
			return "yield " + e.expr(v.Value)
		}
		return "yield"
	case *ast.TypeExpression:
		return v.Name
	case *ast.TupleLiteral:
		parts := make([]string, 0, len(v.Elements))
		for _, el := range v.Elements {
			parts = append(parts, e.expr(el))
		}
		return "(" + strings.Join(parts, ", ") + ")"
	case *ast.SetLiteral:
		parts := make([]string, 0, len(v.Elements))
		for _, el := range v.Elements {
			parts = append(parts, e.expr(el))
		}
		if e.target == "python" {
			return "{" + strings.Join(parts, ", ") + "}"
		}
		if e.target == "javascript" {
			return "new Set([" + strings.Join(parts, ", ") + "])"
		}
		if e.target == "ruby" {
			return "[" + strings.Join(parts, ", ") + "].to_set"
		}
		return "[]interface{}{" + strings.Join(parts, ", ") + "}"
	default:
		if s, ok := x.(*ast.ArrayLiteral); ok {
			return e.arrayLit(s)
		}
		return "null"
	}
}

func (e *emitter) args(list []ast.Expression) string {
	parts := make([]string, 0, len(list))
	for _, a := range list {
		parts = append(parts, e.expr(a))
	}
	return strings.Join(parts, ", ")
}

func (e *emitter) boolLit(v bool) string {
	if e.target == "python" {
		if v {
			return "True"
		}
		return "False"
	}
	if v {
		return "true"
	}
	return "false"
}

func (e *emitter) nilLit() string {
	switch e.target {
	case "python":
		return "None"
	case "javascript", "java":
		return "null"
	}
	return "nil"
}

var printBuiltin = map[string]map[string]string{
	"python":     {"print": "print", "println": "print"},
	"javascript": {"print": "console.log", "println": "console.log"},
	"java":       {"print": "System.out.println", "println": "System.out.println"},
	"ruby":       {"print": "puts", "println": "puts"},
	"go":         {"print": "fmt.Println", "println": "fmt.Println"},
}

func (e *emitter) callExpr(c *ast.CallExpression) string {
	name := ""
	if id, ok := c.Function.(*ast.Identifier); ok {
		name = id.Value
	} else {
		return e.expr(c.Function) + "(" + e.args(c.Arguments) + ")"
	}
	if mapped, ok := printBuiltin[e.target][name]; ok {
		return mapped + "(" + e.args(c.Arguments) + ")"
	}
	if e.kw != nil {
		if mapped, ok := e.kw[name]; ok && strings.TrimSpace(mapped) != "" {
			return mapped + "(" + e.args(c.Arguments) + ")"
		}
	}
	return name + "(" + e.args(c.Arguments) + ")"
}

func (e *emitter) prefix(p *ast.PrefixExpression) string {
	op := p.Operator
	if op == "!" && e.target == "python" {
		op = "not "
	}
	return "(" + op + " " + e.expr(p.Right) + ")"
}

func (e *emitter) infix(i *ast.InfixExpression) string {
	left := e.expr(i.Left)
	right := e.expr(i.Right)
	op := i.Operator
	switch e.target {
	case "python":
		switch op {
		case "&&":
			op = "and"
		case "||":
			op = "or"
		case "===":
			op = "=="
		case "!==":
			op = "!="
		case "++":
			return left + " += 1"
		case "--":
			return left + " -= 1"
		}
	case "javascript", "go", "java", "ruby":
		switch op {
		case "===":
			op = "=="
		case "!==":
			op = "!="
		}
	}
	if op == "++" || op == "--" {
		return left + " " + op
	}
	return left + " " + op + " " + right
}

func (e *emitter) ternary(t *ast.TernaryExpression) string {
	if e.target == "python" {
		return e.expr(t.Consequence) + " if " + e.expr(t.Condition) + " else " + e.expr(t.Alternative)
	}
	return e.expr(t.Condition) + " ? " + e.expr(t.Consequence) + " : " + e.expr(t.Alternative)
}

func (e *emitter) coalesce(c *ast.NullCoalescingExpression) string {
	switch e.target {
	case "python":
		return e.expr(c.Left) + " if " + e.expr(c.Left) + " is not None else " + e.expr(c.Right)
	case "javascript":
		return e.expr(c.Left) + " ?? " + e.expr(c.Right)
	case "java":
		return "(" + e.expr(c.Left) + " != null ? " + e.expr(c.Left) + " : " + e.expr(c.Right) + ")"
	case "ruby":
		return e.expr(c.Left) + " || " + e.expr(c.Right)
	}
	return e.expr(c.Left) + " ?? " + e.expr(c.Right)
}

func (e *emitter) lambda(l *ast.LambdaExpression) string {
	params := make([]string, 0, len(l.Parameters))
	for _, p := range l.Parameters {
		params = append(params, p.Name.Value)
	}
	pstr := strings.Join(params, ", ")
	switch e.target {
	case "python":
		if l.Expression != nil {
			return "lambda " + pstr + ": " + e.expr(l.Expression)
		}
		return "lambda " + pstr + ": None"
	case "javascript":
		if l.Expression != nil {
			return "(" + pstr + ") => " + e.expr(l.Expression)
		}
		return "(" + pstr + ") => {\\n" + e.stmtsText(l.Body.Statements, 1) + "\\n}"
	case "ruby":
		return "lambda { |" + pstr + "| " + e.stmtsText(l.Body.Statements, 0) + " }"
	case "java":
		if l.Expression != nil {
			return "(" + pstr + ") -> " + e.expr(l.Expression)
		}
		return "(" + pstr + ") -> { " + e.stmtsText(l.Body.Statements, 0) + " }"
	}
	return "func(" + pstr + ") { " + e.stmtsText(l.Body.Statements, 0) + " }"
}

// stmtsText joins statements right after each other (same-line lambdas).
func (e *emitter) stmtsText(list []ast.Statement, depth int) string {
	lines := e.statements(list, depth)
	return strings.Join(lines, "\n")
}

func (e *emitter) statements(list []ast.Statement, depth int) []string {
	lines := make([]string, 0, len(list))
	for _, s := range list {
		for _, piece := range e.statement(s, depth) {
			if piece != "" {
				lines = append(lines, piece)
			}
		}
	}
	return lines
}

// ---------------------------------------------------------------------------
// Expressions: array / map / comprehension per target
// ---------------------------------------------------------------------------

func (e *emitter) arrayLit(a *ast.ArrayLiteral) string {
	parts := make([]string, 0, len(a.Elements))
	for _, el := range a.Elements {
		parts = append(parts, e.expr(el))
	}
	switch e.target {
	case "go":
		return "[]interface{}{" + strings.Join(parts, ", ") + "}"
	case "java":
		return "new Object[]{" + strings.Join(parts, ", ") + "}"
	}
	return "[" + strings.Join(parts, ", ") + "]"
}

func (e *emitter) mapLit(m *ast.MapLiteral) string {
	var pairs []string
	for _, p := range m.Pairs {
		pairs = append(pairs, e.mapPair(p))
	}
	switch e.target {
	case "go":
		return "map[string]interface{}{" + strings.Join(pairs, ", ") + "}"
	case "python":
		return "{" + strings.Join(pairs, ", ") + "}"
	case "javascript":
		return "{" + strings.Join(pairs, ", ") + "}"
	case "java":
		puts := make([]string, 0, len(m.Pairs))
		for _, p := range m.Pairs {
			puts = append(puts, "put("+e.expr(p.Key)+", "+e.expr(p.Value)+")")
		}
		return "new java.util.HashMap<String,Object>() {{ " + strings.Join(puts, "; ") + "; }}"
	case "ruby":
		return "{" + strings.Join(pairs, ", ") + "}"
	}
	return "{" + strings.Join(pairs, ", ") + "}"
}

func (e *emitter) mapPair(p *ast.MapPair) string {
	k := p.Key
	// Keep string keys quoted in JS to stay a valid object literal.
	v := e.expr(p.Value)
	switch e.target {
	case "python":
		return e.expr(k) + ": " + v
	case "javascript":
		return e.expr(k) + ": " + v
	case "ruby":
		return e.expr(k) + " => " + v
	}
	return e.expr(k) + ": " + v
}

func (e *emitter) comprehension(lc *ast.ListComprehension) string {
	item := "item"
	if lc.Variable != nil {
		item = lc.Variable.Value
	}
	iter := e.expr(lc.Iterable)
	el := e.expr(lc.Element)
	if lc.Condition != nil {
		cond := e.expr(lc.Condition)
		switch e.target {
		case "python":
			return "[" + el + " for " + item + " in " + iter + " if " + cond + "]"
		case "javascript":
			return iter + ".filter(" + item + " => " + cond + ").map(" + item + " => " + el + ")"
		case "ruby":
			return iter + ".select { |" + item + "| " + cond + " }.map { |" + item + "| " + el + " }"
		case "go":
			return "[" + el + " for " + item + " in " + iter + " if " + cond + "]"
		}
		return "/* list comprehension */ new Object[0]"
	}
	switch e.target {
	case "python":
		return "[" + el + " for " + item + " in " + iter + "]"
	case "javascript":
		return iter + ".map(" + item + " => " + el + ")"
	case "ruby":
		return iter + ".map { |" + item + "| " + el + " }"
	case "go":
		return "[" + el + " for " + item + " in " + iter + "]"
	}
	return "/* list comprehension */ new Object[0]"
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

func (e *emitter) statement(s ast.Statement, depth int) []string {
	if s == nil {
		return nil
	}
	ind := e.ind(depth)
	switch v := s.(type) {
	case *ast.ExpressionStatement:
		line := ind + e.expr(v.Expression)
		if e.target == "java" {
			line += ";"
		}
		return []string{line}
	case *ast.VariableStatement:
		return []string{ind + e.varDecl(v, depth)}
	case *ast.AssignmentStatement:
		line := ind + e.expr(v.Name) + " " + v.Operator + " " + e.expr(v.Value)
		if e.target == "java" {
			line += ";"
		}
		return []string{line}
	case *ast.IfStatement:
		return e.ifStmt(v, depth)
	case *ast.ForStatement:
		return e.forIn(v, depth)
	case *ast.ForEachStatement:
		return e.foreachStmt(v, depth)
	case *ast.ForClassicStatement:
		return e.forClassic(v, depth)
	case *ast.WhileStatement:
		return e.whileStmt(v, depth)
	case *ast.DoWhileStatement:
		return e.doWhile(v, depth)
	case *ast.FunctionStatement:
		return []string{ind + e.function(v, depth)}
	case *ast.ReturnStatement:
		if v.Value != nil {
			return []string{ind + "return " + e.expr(v.Value)}
		}
		if e.target == "java" {
			return []string{ind + "return null;"}
		}
		return []string{ind + "return"}
	case *ast.BreakStatement:
		return []string{ind + "break"}
	case *ast.ContinueStatement:
		return []string{ind + "continue"}
	case *ast.ThrowStatement:
		return []string{ind + e.throwStmt(v)}
	case *ast.TryStatement:
		return e.tryStmt(v, depth)
	case *ast.MatchStatement:
		return e.matchStmt(v, depth)
	case *ast.ClassStatement:
		return e.classStmt(v, depth)
	case *ast.ImportStatement:
		return e.importStmt(v)
	case *ast.ExportStatement:
		return nil
	}
	return nil
}

func (e *emitter) varDecl(v *ast.VariableStatement, depth int) string {
	val := e.expr(v.Value)
	ind := e.ind(depth)
	switch e.target {
	case "go":
		return ind + "var " + v.Name.Value + " = " + val
	case "javascript":
		kw := "let"
		if !v.Mut {
			kw = "const"
		}
		return ind + kw + " " + v.Name.Value + " = " + val
	case "java":
		return ind + "var " + v.Name.Value + " = " + val + ";"
	}
	return ind + v.Name.Value + " = " + val
}

func (e *emitter) function(f *ast.FunctionStatement, depth int) string {
	params := make([]string, 0, len(f.Parameters))
	for _, p := range f.Parameters {
		params = append(params, p.Name.Value)
	}
	pstr := strings.Join(params, ", ")
	body := e.statements(f.Body.Statements, depth+1)
	ind := e.ind(depth)
	switch e.target {
	case "python":
		kw := "def"
		if f.IsAsync {
			kw = "async def"
		}
		out := ind + kw + " " + f.Name.Value + "(" + pstr + "):\n"
		if len(body) == 0 {
			return out + ind + indentUnit + "pass"
		}
		return out + strings.Join(body, "\n")
	case "javascript":
		kw := "function"
		if f.IsAsync {
			kw = "async function"
		}
		out := ind + kw + " " + f.Name.Value + "(" + pstr + ") {\n"
		if len(body) > 0 {
			out += strings.Join(body, "\n") + "\n"
		}
		return out + ind + "}"
	case "ruby":
		out := ind + "def " + f.Name.Value + "(" + pstr + ")\n"
		if len(body) > 0 {
			out += strings.Join(body, "\n") + "\n"
		}
		return out + ind + "end"
	case "java":
		method := e.javaFunction(f, depth)
		return method
	}
	// go
	out := ind + "func " + f.Name.Value + "(" + pstr + ") {\n"
	if len(body) > 0 {
		out += strings.Join(body, "\n") + "\n"
	}
	return out + ind + "}"
}

func (e *emitter) javaFunction(f *ast.FunctionStatement, depth int) string {
	params := make([]string, 0, len(f.Parameters))
	for _, p := range f.Parameters {
		params = append(params, "Object "+p.Name.Value)
	}
	pstr := strings.Join(params, ", ")
	ind := e.ind(depth)
	name := f.Name.Value
	head := ind + "static " + javaReturnType(f) + " " + name + "(" + pstr + ") {"
	if name == "main" {
		head = ind + "public static void main(String[] args) {"
	}
	body := e.statements(f.Body.Statements, depth+1)
	out := head
	if len(body) == 0 {
		return out + "\n" + ind + "}"
	}
	out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
	return out
}

func javaReturnType(f *ast.FunctionStatement) string {
	if f.Name != nil && f.Name.Value == "main" {
		return "void"
	}
	for _, s := range f.Body.Statements {
		if r, ok := s.(*ast.ReturnStatement); ok && r.Value != nil {
			return "Object"
		}
	}
	return "void"
}

func (e *emitter) ifStmt(i *ast.IfStatement, depth int) []string {
	ind := e.ind(depth)
	cond := e.expr(i.Condition)
	body := e.statements(i.Consequence.Statements, depth+1)
	var out []string
	switch e.target {
	case "python":
		out = append(out, ind+"if "+cond+":")
		if len(body) == 0 {
			out = append(out, e.ind(depth+1)+"pass")
		} else {
			out = append(out, body...)
		}
	case "ruby":
		out = append(out, ind+"if "+cond)
		out = append(out, body...)
	case "javascript", "java", "go":
		block := ind + "if (" + cond + ") {"
		if len(body) > 0 {
			block += "\n" + strings.Join(body, "\n")
		}
		out = append(out, block)
	}
	e.appendElse(i.Alternative, &out, depth, cond)
	return out
}

func (e *emitter) appendElse(alt ast.Statement, out *[]string, depth int, cond string) {
	ind := e.ind(depth)
	switch a := alt.(type) {
	case nil:
		if e.target == "ruby" {
			*out = append(*out, ind+"end")
		} else if e.target == "javascript" || e.target == "java" || e.target == "go" {
			*out = append(*out, ind+"}")
		}
		return
	case *ast.IfStatement:
		nested := e.ifStmt(a, depth)
		switch e.target {
		case "python":
			// nested.ifStmt emits `ind if ...`; rewrite first line to `elif`. We know
			// the emitted first line is exactly `ind + "if ..."`.
		case "ruby":
			*out = append(*out, ind+"elsif "+e.expr(a.Condition))
			nestedBody := e.statements(a.Consequence.Statements, depth+1)
			*out = append(*out, nestedBody...)
			e.appendElse(a.Alternative, out, depth, cond)
			return
		case "javascript", "java", "go":
			// `else if` in one line; closing brace handled by recursion
			block := ind + "} else if (" + e.expr(a.Condition) + ") {"
			if b := e.statements(a.Consequence.Statements, depth+1); len(b) > 0 {
				block += "\n" + strings.Join(b, "\n")
			}
			*out = append(*out, block)
			e.appendElse(a.Alternative, out, depth, cond)
			return
		}
		if e.target == "python" {
			for idx, line := range nested {
				if strings.HasPrefix(line, ind+"if ") {
					nested[idx] = ind + "elif " + line[len(ind+"if "):]
					break
				}
			}
			*out = append(*out, nested...)
		}
		return
	case *ast.BlockStatement:
		body := e.statements(a.Statements, depth+1)
		switch e.target {
		case "python":
			*out = append(*out, ind+"else:")
			if len(body) == 0 {
				*out = append(*out, e.ind(depth+1)+"pass")
			} else {
				*out = append(*out, body...)
			}
		case "ruby":
			*out = append(*out, ind+"else")
			*out = append(*out, body...)
			*out = append(*out, ind+"end")
		case "javascript", "java", "go":
			block := ind + "} else {"
			if len(body) == 0 {
				block += " }"
			} else {
				block += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
			}
			*out = append(*out, block)
		}
		return
	}
}

func (e *emitter) forIn(f *ast.ForStatement, depth int) []string {
	ind := e.ind(depth)
	item := "item"
	if f.Item != nil {
		item = f.Item.Value
	}
	iter := e.expr(f.Iterable)
	body := e.statements(f.Body.Statements, depth+1)
	switch e.target {
	case "python":
		out := []string{ind + "for " + item + " in " + iter + ":"}
		if len(body) == 0 {
			return append(out, e.ind(depth+1)+"pass")
		}
		return append(out, body...)
	case "javascript":
		out := ind + "for (const " + item + " of " + iter + ") {"
		if len(body) == 0 {
			out += " }"
			return []string{out}
		}
		out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{out}
	case "java":
		out := ind + "for (Object " + item + " : " + iter + ") {"
		if len(body) == 0 {
			out += " }"
			return []string{out}
		}
		out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{out}
	case "ruby":
		out := []string{ind + "for " + item + " in " + iter}
		out = append(out, body...)
		out = append(out, ind+"end")
		return out
	case "go":
		out := ind + "for _, " + item + " := range " + iter + " {"
		if len(body) == 0 {
			out += " }"
			return []string{out}
		}
		out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{out}
	}
	out := ind + "for " + item + " in " + iter + " {"
	if len(body) == 0 {
		out += " }"
		return []string{out}
	}
	out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
	return []string{out}
}

func (e *emitter) foreachStmt(f *ast.ForEachStatement, depth int) []string {
	ind := e.ind(depth)
	key := "key"
	val := "value"
	if f.Key != nil {
		key = f.Key.Value
	}
	if f.Value != nil {
		val = f.Value.Value
	}
	obj := e.expr(f.Object)
	body := e.statements(f.Body.Statements, depth+1)
	switch e.target {
	case "python":
		out := []string{ind + "for " + key + ", " + val + " in " + obj + ".items():"}
		if len(body) == 0 {
			return append(out, e.ind(depth+1)+"pass")
		}
		return append(out, body...)
	case "javascript":
		out := ind + "for (const [" + key + ", " + val + "] of Object.entries(" + obj + ")) {"
		if len(body) == 0 {
			out += " }"
			return []string{out}
		}
		out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{out}
	case "ruby":
		out := []string{ind + obj + ".each do |" + key + ", " + val + "|"}
		out = append(out, body...)
		out = append(out, ind+"end")
		return out
	case "go":
		out := ind + "for " + key + ", " + val + " := range " + obj + " {"
		if len(body) == 0 {
			out += " }"
			return []string{out}
		}
		out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{out}
	}
	out := ind + "for " + key + ", " + val + " in " + obj + " {"
	if len(body) == 0 {
		out += " }"
		return []string{out}
	}
	out += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
	return []string{out}
}

func (e *emitter) forClassic(f *ast.ForClassicStatement, depth int) []string {
	ind := e.ind(depth)
	body := e.statements(f.Body.Statements, depth+1)
	init := ""
	if f.Init != nil {
		parts := e.statement(f.Init, 0)
		init = strings.TrimSpace(strings.Join(parts, "\n"))
	}
	cond := ""
	if f.Condition != nil {
		cond = e.expr(f.Condition)
	}
	update := ""
	if f.Update != nil {
		parts := e.statement(f.Update, 0)
		update = strings.TrimSpace(strings.Join(parts, "\n"))
	}
	switch e.target {
	case "python":
		var out []string
		if init != "" {
			out = append(out, ind+init)
		}
		out = append(out, ind+"while "+cond+":")
		if len(body) == 0 {
			out = append(out, e.ind(depth+1)+"pass")
		} else {
			out = append(out, body...)
		}
		if update != "" {
			out = append(out, ind+update)
		}
		return out
	case "ruby":
		var out []string
		if init != "" {
			out = append(out, ind+init)
		}
		out = append(out, ind+"while "+cond)
		out = append(out, body...)
		if update != "" {
			out = append(out, ind+update)
		}
		out = append(out, ind+"end")
		return out
	case "go":
		line := ind + "for " + init + "; " + cond + "; " + update + " {"
		if len(body) == 0 {
			line += " }"
			return []string{line}
		}
		line += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{line}
	}
	line := ind + "for (" + init + "; " + cond + "; " + update + ") {"
	if len(body) == 0 {
		line += " }"
		return []string{line}
	}
	line += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
	return []string{line}
}

func (e *emitter) whileStmt(w *ast.WhileStatement, depth int) []string {
	ind := e.ind(depth)
	cond := e.expr(w.Condition)
	body := e.statements(w.Body.Statements, depth+1)
	switch e.target {
	case "python":
		out := []string{ind + "while " + cond + ":"}
		if len(body) == 0 {
			return append(out, e.ind(depth+1)+"pass")
		}
		return append(out, body...)
	case "ruby":
		out := []string{ind + "while " + cond}
		out = append(out, body...)
		out = append(out, ind+"end")
		return out
	case "go":
		line := ind + "for " + cond + " {"
		if len(body) == 0 {
			line += " }"
			return []string{line}
		}
		line += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
		return []string{line}
	}
	line := ind + "while (" + cond + ") {"
	if len(body) == 0 {
		line += " }"
		return []string{line}
	}
	line += "\n" + strings.Join(body, "\n") + "\n" + ind + "}"
	return []string{line}
}

func (e *emitter) doWhile(d *ast.DoWhileStatement, depth int) []string {
	ind := e.ind(depth)
	cond := e.expr(d.Condition)
	body := e.statements(d.Body.Statements, depth+1)
	switch e.target {
	case "python":
		// Python has no do-while: emit the body once, then loop on the condition.
		out := append([]string{}, body...)
		out = append(out, ind+"while "+cond+":")
		if len(body) == 0 {
			out = append(out, e.ind(depth+1)+"pass")
		} else {
			out = append(out, body...)
		}
		return out
	case "ruby":
		out := append([]string{ind + "begin"}, body...)
		out = append(out, ind+"end while "+cond)
		return out
	}
	line := ind + "do {"
	if len(body) == 0 {
		line += " } while (" + cond + ");"
		return []string{line}
	}
	line += "\n" + strings.Join(body, "\n") + "\n" + ind + "} while (" + cond + ");"
	return []string{line}
}

func (e *emitter) throwStmt(t *ast.ThrowStatement) string {
	v := e.expr(t.Value)
	switch e.target {
	case "python":
		return "raise " + v
	case "java", "javascript":
		return "throw " + v + ";"
	case "ruby":
		return "raise " + v
	}
	return "throw " + v
}

func (e *emitter) tryStmt(t *ast.TryStatement, depth int) []string {
	ind := e.ind(depth)
	tryBody := e.statements(t.TryBlock.Statements, depth+1)
	var out []string
	switch e.target {
	case "python":
		out = append(out, ind+"try:")
		if len(tryBody) == 0 {
			out = append(out, e.ind(depth+1)+"pass")
		} else {
			out = append(out, tryBody...)
		}
		for _, c := range t.Catches {
			exc := "Exception"
			if c.Type != nil {
				exc = c.Type.Name
			}
			head := ind + "except " + exc
			if c.Variable != nil {
				head += " as " + c.Variable.Value
			}
			head += ":"
			out = append(out, head)
			body := e.statements(c.Body.Statements, depth+1)
			if len(body) == 0 {
				out = append(out, e.ind(depth+1)+"pass")
			} else {
				out = append(out, body...)
			}
		}
		if t.Finally != nil {
			fin := e.statements(t.Finally.Statements, depth+1)
			out = append(out, ind+"finally:")
			if len(fin) == 0 {
				out = append(out, e.ind(depth+1)+"pass")
			} else {
				out = append(out, fin...)
			}
		}
		return out
	case "ruby":
		out = append(out, ind+"begin")
		out = append(out, tryBody...)
		for _, c := range t.Catches {
			head := ind + "rescue"
			if c.Type != nil {
				head += " " + c.Type.Name
			}
			if c.Variable != nil {
				head += " => " + c.Variable.Value
			}
			out = append(out, head)
			out = append(out, e.statements(c.Body.Statements, depth+1)...)
		}
		if t.Finally != nil {
			out = append(out, ind+"ensure")
			out = append(out, e.statements(t.Finally.Statements, depth+1)...)
		}
		out = append(out, ind+"end")
		return out
	}
	// javascript / java / go
	out = append(out, ind+"try {")
	if len(tryBody) > 0 {
		out = append(out, tryBody...)
	}
	for _, c := range t.Catches {
		varName := "e"
		if c.Variable != nil {
			varName = c.Variable.Value
		}
		body := e.statements(c.Body.Statements, depth+1)
		out = append(out, ind+"} catch ("+varName+") {")
		if len(body) > 0 {
			out = append(out, body...)
		}
	}
	if t.Finally != nil {
		fin := e.statements(t.Finally.Statements, depth+1)
		out = append(out, ind+"} finally {")
		if len(fin) > 0 {
			out = append(out, fin...)
		}
		out = append(out, ind+"}")
	} else {
		out = append(out, ind+"}")
	}
	return out
}

func (e *emitter) matchStmt(m *ast.MatchStatement, depth int) []string {
	ind := e.ind(depth)
	subject := e.expr(m.Subject)
	switch e.target {
	case "python":
		var out []string
		for i, c := range m.Cases {
			kw := "if"
			if i > 0 {
				kw = "elif"
			}
			out = append(out, ind+kw+" "+subject+" == "+e.expr(c.Pattern)+":")
			body := e.statements(c.Consequence.Statements, depth+1)
			if len(body) == 0 {
				out = append(out, e.ind(depth+1)+"pass")
			} else {
				out = append(out, body...)
			}
		}
		if m.Default != nil {
			out = append(out, ind+"else:")
			dbody := e.statements(m.Default.Statements, depth+1)
			if len(dbody) == 0 {
				out = append(out, e.ind(depth+1)+"pass")
			} else {
				out = append(out, dbody...)
			}
		} else if len(m.Cases) == 0 {
			out = append(out, ind+"pass")
		}
		return out
	case "ruby":
		out := []string{ind + "case " + subject}
		for _, c := range m.Cases {
			out = append(out, ind+"when "+e.expr(c.Pattern))
			out = append(out, e.statements(c.Consequence.Statements, depth+1)...)
		}
		if m.Default != nil {
			out = append(out, ind+"else")
			out = append(out, e.statements(m.Default.Statements, depth+1)...)
		}
		out = append(out, ind+"end")
		return out
	}
	// javascript / java / go switch
	line := ind + "switch (" + subject + ") {"
	out := []string{line}
	for _, c := range m.Cases {
		out = append(out, ind+"case "+e.expr(c.Pattern)+":")
		out = append(out, e.statements(c.Consequence.Statements, depth+1)...)
		if e.target == "javascript" || e.target == "java" {
			out = append(out, ind+"break;")
		}
	}
	if m.Default != nil {
		out = append(out, ind+"default:")
		out = append(out, e.statements(m.Default.Statements, depth+1)...)
	} else if e.target == "javascript" || e.target == "java" {
		// no default; keep switch from falling through
		out = append(out, ind+"default: break;")
	}
	out = append(out, ind+"}")
	return out
}

func (e *emitter) classStmt(c *ast.ClassStatement, depth int) []string {
	ind := e.ind(depth)
	switch e.target {
	case "python":
		head := ind + "class " + c.Name.Value + ":"
		if c.Extends != nil {
			head = ind + "class " + c.Name.Value + "(" + c.Extends.Value + "):"
		}
		var inner []string
		for _, p := range c.Properties {
			inner = append(inner, e.ind(depth+1)+p.Name.Value+" = "+e.expr(p.Value))
		}
		for _, m := range c.Methods {
			inner = append(inner, e.function(m, depth+1))
		}
		if len(inner) == 0 {
			return []string{head, e.ind(depth+1) + "pass"}
		}
		return append([]string{head}, inner...)
	case "ruby":
		head := ind + "class " + c.Name.Value
		if c.Extends != nil {
			head += " < " + c.Extends.Value
		}
		var inner []string
		for _, m := range c.Methods {
			inner = append(inner, e.function(m, depth+1))
		}
		inner = append(inner, ind+"end")
		return append([]string{head}, inner...)
	case "javascript":
		head := ind + "class " + c.Name.Value + " {"
		if c.Extends != nil {
			head = ind + "class " + c.Name.Value + " extends " + c.Extends.Value + " {"
		}
		var inner []string
		for _, m := range c.Methods {
			inner = append(inner, e.ind(depth+1)+m.Name.Value+"("+e.funcParams(m)+") {")
			inner = append(inner, e.statements(m.Body.Statements, depth+2)...)
			inner = append(inner, e.ind(depth+1)+"}")
		}
		inner = append(inner, ind+"}")
		return append([]string{head}, inner...)
	}
	// go / java: keep the class shape for "go"; java emits a proper class shell.
	head := ind + "class " + c.Name.Value
	if c.Extends != nil {
		head += " extends " + c.Extends.Value
	}
	head += " {"
	var inner []string
	for _, p := range c.Properties {
		if e.target == "java" {
			inner = append(inner, e.ind(depth+1)+"Object "+p.Name.Value+" = "+e.expr(p.Value)+";")
		} else {
			inner = append(inner, e.ind(depth+1)+p.Name.Value+" = "+e.expr(p.Value))
		}
	}
	for _, m := range c.Methods {
		inner = append(inner, e.function(m, depth+1))
	}
	inner = append(inner, ind+"}")
	return append([]string{head}, inner...)
}

func (e *emitter) funcParams(f *ast.FunctionStatement) string {
	parts := make([]string, 0, len(f.Parameters))
	for _, p := range f.Parameters {
		parts = append(parts, p.Name.Value)
	}
	return strings.Join(parts, ", ")
}

func (e *emitter) importStmt(i *ast.ImportStatement) []string {
	mod := ""
	if i.Module != nil {
		mod = i.Module.Value
	}
	if mod == "" {
		return nil
	}
	switch e.target {
	case "python":
		return []string{"import " + mod}
	case "javascript":
		if i.Alias != nil {
			return []string{"import " + i.Alias.Value + " from '" + mod + "'"}
		}
		if len(i.Items) > 0 {
			var names []string
			for _, it := range i.Items {
				names = append(names, it.Value)
			}
			return []string{"import { " + strings.Join(names, ", ") + " } from '" + mod + "'"}
		}
		return []string{"import '" + mod + "'"}
	case "java":
		return []string{"import " + mod + ";"}
	case "ruby":
		return []string{"require '" + mod + "'"}
	}
	return []string{"// import " + mod}
}

// ---------------------------------------------------------------------------
// Complete file builders, one per target
// ---------------------------------------------------------------------------

func (e *emitter) emitGo(prog *ast.Program) string {
	var funcs, classes, body []string
	hasMain := false
	for _, s := range prog.Statements {
		switch v := s.(type) {
		case *ast.FunctionStatement:
			funcs = append(funcs, e.function(v, 0))
			if v.Name != nil && v.Name.Value == "main" {
				hasMain = true
			}
		case *ast.ClassStatement:
			classes = append(classes, e.classStmt(v, 0)...)
		default:
			body = append(body, e.labeledStatement(v, 0)...)
		}
	}
	usesFmt := e.goUsesFmt(prog)

	var out strings.Builder
	out.WriteString("package main\n")
	if usesFmt {
		out.WriteString("import \"fmt\"\n")
	}
	out.WriteString("\n")
	for _, f := range funcs {
		out.WriteString(f + "\n\n")
	}
	for _, c := range classes {
		out.WriteString(c + "\n\n")
	}
	if len(body) > 0 {
		wrapper := "func main() {\n"
		if hasMain {
			wrapper = "func init() {\n"
		}
		out.WriteString(wrapper)
		for _, b := range body {
			out.WriteString(prefixLines(b, "    ") + "\n")
		}
		out.WriteString("}\n")
	}
	return strings.TrimRight(out.String(), "\n")
}

// prefixLines indents every line of s with prefix (multi-line aware).
func prefixLines(s, prefix string) string {
	return prefix + strings.Join(strings.Split(s, "\n"), "\n"+prefix)
}

// labeledStatement emits a statement plus (for expression statements) nothing
// extra. Kept as a small indirection so emitGo can mimic wrapper placement.
func (e *emitter) labeledStatement(s ast.Statement, depth int) []string {
	return e.statement(s, depth)
}

func (e *emitter) goUsesFmt(prog *ast.Program) bool {
	if e.target != "go" {
		return false
	}
	visit := func(s ast.Statement) bool {
		switch v := s.(type) {
		case *ast.ExpressionStatement:
			return e.exprUsesFmt(v.Expression)
		case *ast.VariableStatement:
			return e.exprUsesFmt(v.Value)
		case *ast.AssignmentStatement:
			return e.exprUsesFmt(v.Value)
		}
		return false
	}
	var walkBlock func(b *ast.BlockStatement) bool
	var walkStmt func(s ast.Statement) bool
	walkBlock = func(b *ast.BlockStatement) bool {
		if b == nil {
			return false
		}
		for _, s := range b.Statements {
			if walkStmt(s) {
				return true
			}
		}
		return false
	}
	walkStmt = func(s ast.Statement) bool {
		if visit(s) {
			return true
		}
		switch v := s.(type) {
		case *ast.FunctionStatement:
			return walkBlock(v.Body)
		case *ast.IfStatement:
			return e.exprUsesFmt(v.Condition) || walkBlock(v.Consequence)
		case *ast.ForStatement:
			return e.exprUsesFmt(v.Iterable) || walkBlock(v.Body)
		case *ast.WhileStatement:
			return e.exprUsesFmt(v.Condition) || walkBlock(v.Body)
		case *ast.TryStatement:
			return walkBlock(v.TryBlock)
		case *ast.MatchStatement:
			return e.exprUsesFmt(v.Subject)
		}
		return false
	}
	for _, s := range prog.Statements {
		if walkStmt(s) {
			return true
		}
	}
	return false
}

func (e *emitter) exprUsesFmt(x ast.Expression) bool {
	if x == nil {
		return false
	}
	switch v := x.(type) {
	case *ast.CallExpression:
		if id, ok := v.Function.(*ast.Identifier); ok {
			if id.Value == "print" || id.Value == "println" || strings.HasPrefix(id.Value, "fmt.") {
				return true
			}
			if mapped, ok := e.kw[id.Value]; ok && strings.Contains(mapped, "fmt") {
				return true
			}
		}
		for _, a := range v.Arguments {
			if e.exprUsesFmt(a) {
				return true
			}
		}
	case *ast.MethodCallExpression:
		for _, a := range v.Arguments {
			if e.exprUsesFmt(a) {
				return true
			}
		}
	case *ast.InfixExpression:
		return e.exprUsesFmt(v.Left) || e.exprUsesFmt(v.Right)
	case *ast.PrefixExpression:
		return e.exprUsesFmt(v.Right)
	case *ast.IndexExpression:
		return e.exprUsesFmt(v.Left) || e.exprUsesFmt(v.Index)
	case *ast.ArrayLiteral:
		for _, el := range v.Elements {
			if e.exprUsesFmt(el) {
				return true
			}
		}
	case *ast.MapLiteral:
		for _, p := range v.Pairs {
			if e.exprUsesFmt(p.Key) || e.exprUsesFmt(p.Value) {
				return true
			}
		}
	}
	return false
}

func (e *emitter) emitPython(prog *ast.Program) string {
	lines := e.statements(prog.Statements, 0)
	return strings.Join(lines, "\n")
}

func (e *emitter) emitJavaScript(prog *ast.Program) string {
	lines := e.statements(prog.Statements, 0)
	return strings.Join(lines, "\n")
}

func (e *emitter) emitRuby(prog *ast.Program) string {
	lines := e.statements(prog.Statements, 0)
	return strings.Join(lines, "\n")
}

func (e *emitter) emitJava(prog *ast.Program) string {
	var funcs, classes, body []string
	hasMain := false
	for _, s := range prog.Statements {
		switch v := s.(type) {
		case *ast.FunctionStatement:
			funcs = append(funcs, e.javaFunction(v, 1))
			if v.Name != nil && v.Name.Value == "main" {
				hasMain = true
			}
		case *ast.ClassStatement:
			classes = append(classes, e.classStmt(v, 1)...)
		default:
			body = append(body, e.statement(s, 0)...)
		}
	}
	var out strings.Builder
	out.WriteString("public class Main {\n")
	for _, f := range funcs {
		out.WriteString(f + "\n")
	}
	for _, c := range classes {
		out.WriteString(c + "\n")
	}
	if !hasMain {
		out.WriteString("    public static void main(String[] args) {\n")
		for _, b := range body {
			out.WriteString(prefixLines(b, "        ") + "\n")
		}
		out.WriteString("    }\n")
	} else if len(body) > 0 {
		out.WriteString("    static {\n")
		for _, b := range body {
			out.WriteString(prefixLines(b, "        ") + "\n")
		}
		out.WriteString("    }\n")
	}
	out.WriteString("}\n")
	return strings.TrimRight(out.String(), "\n")
}
