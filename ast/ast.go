package ast

import (
	"strconv"
	"strings"
)

type Token struct {
	Type    string
	Literal string
	Line    int
	Column  int
}

type Node interface {
	TokenLiteral() string
}

type Expression interface {
	Node
	expressionNode()
	String() string
}

type Statement interface {
	Node
	statementNode()
	String() string
}

type Program struct {
	Statements []Statement
	Imports    []*ImportStatement
	Functions  []*FunctionStatement
	Classes    []*ClassStatement
}

func (p *Program) TokenLiteral() string { return "" }
func (p *Program) String() string {
	var out strings.Builder
	for _, imp := range p.Imports {
		out.WriteString(imp.String())
		out.WriteByte('\n')
	}
	for _, stmt := range p.Statements {
		out.WriteString(stmt.String())
		out.WriteByte('\n')
	}
	return out.String()
}

// ==================== EXPRESSIONS ====================

type Identifier struct {
	Token Token
	Value string
}

func (i *Identifier) TokenLiteral() string { return i.Token.Literal }
func (i *Identifier) expressionNode()      {}
func (i *Identifier) String() string       { return i.Value }

type IntegerLiteral struct {
	Token Token
	Value int64
}

func (il *IntegerLiteral) TokenLiteral() string { return il.Token.Literal }
func (il *IntegerLiteral) expressionNode()      {}
func (il *IntegerLiteral) String() string {
	return strconv.FormatInt(il.Value, 10)
}

type FloatLiteral struct {
	Token Token
	Value float64
}

func (fl *FloatLiteral) TokenLiteral() string { return fl.Token.Literal }
func (fl *FloatLiteral) expressionNode()      {}
func (fl *FloatLiteral) String() string {
	return strconv.FormatFloat(fl.Value, 'f', -1, 64)
}

type StringLiteral struct {
	Token Token
	Value string
}

func (sl *StringLiteral) TokenLiteral() string { return sl.Token.Literal }
func (sl *StringLiteral) expressionNode()      {}
func (sl *StringLiteral) String() string {
	return "\"" + sl.Value + "\""
}

type TemplateString struct {
	Token    Token
	Parts    []Expression
	RawParts []string
}

func (ts *TemplateString) TokenLiteral() string { return ts.Token.Literal }
func (ts *TemplateString) expressionNode()      {}
func (ts *TemplateString) String() string {
	return "`" + strings.Join(ts.RawParts, "${...}") + "`"
}

type BooleanLiteral struct {
	Token Token
	Value bool
}

func (bl *BooleanLiteral) TokenLiteral() string { return bl.Token.Literal }
func (bl *BooleanLiteral) expressionNode()      {}
func (bl *BooleanLiteral) String() string {
	if bl.Value {
		return "true"
	}
	return "false"
}

type NilLiteral struct {
	Token Token
}

func (nl *NilLiteral) TokenLiteral() string { return nl.Token.Literal }
func (nl *NilLiteral) expressionNode()      {}
func (nl *NilLiteral) String() string       { return "nil" }

type ArrayLiteral struct {
	Token    Token
	Elements []Expression
}

func (al *ArrayLiteral) TokenLiteral() string { return al.Token.Literal }
func (al *ArrayLiteral) expressionNode()      {}
func (al *ArrayLiteral) String() string {
	var els []string
	for _, e := range al.Elements {
		els = append(els, e.String())
	}
	return "[" + strings.Join(els, ", ") + "]"
}

type ListComprehension struct {
	Token     Token
	Element   Expression
	Variable  *Identifier
	Iterable  Expression
	Condition Expression
}

func (lc *ListComprehension) TokenLiteral() string { return lc.Token.Literal }
func (lc *ListComprehension) expressionNode()      {}
func (lc *ListComprehension) String() string {
	if lc.Condition != nil {
		return "[x for " + lc.Variable.String() + " in " + lc.Iterable.String() + " if " + lc.Condition.String() + "]"
	}
	return "[" + lc.Element.String() + " for " + lc.Variable.String() + " in " + lc.Iterable.String() + "]"
}

type TupleLiteral struct {
	Token    Token
	Elements []Expression
}

func (tl *TupleLiteral) TokenLiteral() string { return tl.Token.Literal }
func (tl *TupleLiteral) expressionNode()      {}
func (tl *TupleLiteral) String() string {
	var els []string
	for _, e := range tl.Elements {
		els = append(els, e.String())
	}
	return "(" + strings.Join(els, ", ") + ")"
}

type SetLiteral struct {
	Token    Token
	Elements []Expression
}

func (sl *SetLiteral) TokenLiteral() string { return sl.Token.Literal }
func (sl *SetLiteral) expressionNode()      {}
func (sl *SetLiteral) String() string {
	var els []string
	for _, e := range sl.Elements {
		els = append(els, e.String())
	}
	return "#{" + strings.Join(els, ", ") + "}"
}

type MapLiteral struct {
	Token Token
	Pairs []*MapPair
}

type MapPair struct {
	Key   Expression
	Value Expression
}

func (mp *MapLiteral) TokenLiteral() string { return mp.Token.Literal }
func (mp *MapLiteral) expressionNode()      {}
func (mp *MapLiteral) String() string {
	var pairs []string
	for _, p := range mp.Pairs {
		pairs = append(pairs, p.Key.String()+": "+p.Value.String())
	}
	return "{" + strings.Join(pairs, ", ") + "}"
}

type PrefixExpression struct {
	Token    Token
	Operator string
	Right    Expression
}

func (pe *PrefixExpression) TokenLiteral() string { return pe.Token.Literal }
func (pe *PrefixExpression) expressionNode()      {}
func (pe *PrefixExpression) String() string {
	return "(" + pe.Operator + pe.Right.String() + ")"
}

type InfixExpression struct {
	Token    Token
	Left     Expression
	Operator string
	Right    Expression
}

func (ie *InfixExpression) TokenLiteral() string { return ie.Token.Literal }
func (ie *InfixExpression) expressionNode()      {}
func (ie *InfixExpression) String() string {
	return "(" + ie.Left.String() + " " + ie.Operator + " " + ie.Right.String() + ")"
}

type TernaryExpression struct {
	Token       Token
	Condition   Expression
	Consequence Expression
	Alternative Expression
}

func (te *TernaryExpression) TokenLiteral() string { return te.Token.Literal }
func (te *TernaryExpression) expressionNode()      {}
func (te *TernaryExpression) String() string {
	return te.Condition.String() + " ? " + te.Consequence.String() + " : " + te.Alternative.String()
}

type NullCoalescingExpression struct {
	Token Token
	Left  Expression
	Right Expression
}

func (nc *NullCoalescingExpression) TokenLiteral() string { return nc.Token.Literal }
func (nc *NullCoalescingExpression) expressionNode()      {}
func (nc *NullCoalescingExpression) String() string {
	return "(" + nc.Left.String() + " ?? " + nc.Right.String() + ")"
}

type OptionalChainingExpression struct {
	Token    Token
	Object   Expression
	Property *Identifier
	Index    Expression
	CallArgs []Expression
	IsCall   bool
}

func (oc *OptionalChainingExpression) TokenLiteral() string { return oc.Token.Literal }
func (oc *OptionalChainingExpression) expressionNode()      {}
func (oc *OptionalChainingExpression) String() string {
	return oc.Object.String() + "?." + oc.Property.String()
}

type IndexExpression struct {
	Token Token
	Left  Expression
	Index Expression
}

func (ie *IndexExpression) TokenLiteral() string { return ie.Token.Literal }
func (ie *IndexExpression) expressionNode()      {}
func (ie *IndexExpression) String() string {
	return ie.Left.String() + "[" + ie.Index.String() + "]"
}

type SliceExpression struct {
	Token Token
	Left  Expression
	Start Expression
	End   Expression
	Step  Expression
}

func (se *SliceExpression) TokenLiteral() string { return se.Token.Literal }
func (se *SliceExpression) expressionNode()      {}
func (se *SliceExpression) String() string {
	start := ""
	end := ""
	step := ""
	if se.Start != nil {
		start = se.Start.String()
	}
	if se.End != nil {
		end = se.End.String()
	}
	if se.Step != nil {
		step = ":" + se.Step.String()
	}
	return se.Left.String() + "[" + start + ":" + end + step + "]"
}

type CallExpression struct {
	Token     Token
	Function  Expression
	Arguments []Expression
}

func (ce *CallExpression) TokenLiteral() string { return ce.Token.Literal }
func (ce *CallExpression) expressionNode()      {}
func (ce *CallExpression) String() string {
	var args []string
	for _, a := range ce.Arguments {
		args = append(args, a.String())
	}
	return ce.Function.String() + "(" + strings.Join(args, ", ") + ")"
}

type MethodCallExpression struct {
	Token     Token
	Object    Expression
	Method    *Identifier
	Arguments []Expression
}

func (mc *MethodCallExpression) TokenLiteral() string { return mc.Token.Literal }
func (mc *MethodCallExpression) expressionNode()      {}
func (mc *MethodCallExpression) String() string {
	var args []string
	for _, a := range mc.Arguments {
		args = append(args, a.String())
	}
	return mc.Object.String() + "." + mc.Method.String() + "(" + strings.Join(args, ", ") + ")"
}

type MemberExpression struct {
	Token    Token
	Object   Expression
	Property *Identifier
	Computed bool
}

func (me *MemberExpression) TokenLiteral() string { return me.Token.Literal }
func (me *MemberExpression) expressionNode()      {}
func (me *MemberExpression) String() string {
	if me.Computed {
		return me.Object.String() + "[" + me.Property.String() + "]"
	}
	return me.Object.String() + "." + me.Property.String()
}

type LambdaExpression struct {
	Token      Token
	Parameters []*FunctionParameter
	Body       *BlockStatement
	Expression Expression
	IsAsync    bool
}

func (le *LambdaExpression) TokenLiteral() string { return le.Token.Literal }
func (le *LambdaExpression) expressionNode()      {}
func (le *LambdaExpression) String() string {
	var params []string
	for _, p := range le.Parameters {
		params = append(params, p.Name.Value)
	}
	if le.Expression != nil {
		return "fn(" + strings.Join(params, ", ") + ") => " + le.Expression.String()
	}
	return "fn(" + strings.Join(params, ", ") + ") {" + le.Body.String() + "}"
}

type AwaitExpression struct {
	Token Token
	Value Expression
}

func (ae *AwaitExpression) TokenLiteral() string { return ae.Token.Literal }
func (ae *AwaitExpression) expressionNode()      {}
func (ae *AwaitExpression) String() string {
	return "await " + ae.Value.String()
}

type YieldExpression struct {
	Token Token
	Value Expression
}

func (ye *YieldExpression) TokenLiteral() string { return ye.Token.Literal }
func (ye *YieldExpression) expressionNode()      {}
func (ye *YieldExpression) String() string {
	if ye.Value != nil {
		return "yield " + ye.Value.String()
	}
	return "yield"
}

type SpreadExpression struct {
	Token Token
	Value Expression
}

func (se *SpreadExpression) TokenLiteral() string { return se.Token.Literal }
func (se *SpreadExpression) expressionNode()      {}
func (se *SpreadExpression) String() string {
	return "..." + se.Value.String()
}

type TypeExpression struct {
	Token    Token
	Name     string
	Generics []string
}

func (te *TypeExpression) TokenLiteral() string { return te.Token.Literal }
func (te *TypeExpression) expressionNode()      {}
func (te *TypeExpression) String() string       { return te.Name }

// ==================== STATEMENTS ====================

type BlockStatement struct {
	Token      Token
	Statements []Statement
}

func (bs *BlockStatement) TokenLiteral() string { return bs.Token.Literal }
func (bs *BlockStatement) statementNode()       {}
func (bs *BlockStatement) String() string {
	var out strings.Builder
	out.WriteByte('{')
	for i, s := range bs.Statements {
		if i > 0 {
			out.WriteByte(' ')
		}
		out.WriteString(s.String())
	}
	out.WriteByte('}')
	return out.String()
}

type ExpressionStatement struct {
	Token      Token
	Expression Expression
}

func (es *ExpressionStatement) TokenLiteral() string { return es.Token.Literal }
func (es *ExpressionStatement) statementNode()       {}
func (es *ExpressionStatement) String() string {
	if es.Expression != nil {
		return es.Expression.String()
	}
	return ""
}

type VariableStatement struct {
	Token Token
	Name  *Identifier
	Type  *TypeExpression
	Value Expression
	Mut   bool
}

func (vs *VariableStatement) TokenLiteral() string { return vs.Token.Literal }
func (vs *VariableStatement) statementNode()       {}
func (vs *VariableStatement) String() string {
	m := "val"
	if vs.Mut {
		m = "var"
	}
	if vs.Type != nil {
		return m + " " + vs.Name.String() + ": " + vs.Type.String() + " = " + vs.Value.String()
	}
	return m + " " + vs.Name.String() + " = " + vs.Value.String()
}

type AssignmentStatement struct {
	Token    Token
	Name     Expression
	Operator string
	Value    Expression
}

func (as *AssignmentStatement) TokenLiteral() string { return as.Token.Literal }
func (as *AssignmentStatement) statementNode()       {}
func (as *AssignmentStatement) String() string {
	return as.Name.String() + " " + as.Operator + " " + as.Value.String()
}

type IfStatement struct {
	Token       Token
	Condition   Expression
	Consequence *BlockStatement
	Alternative Statement
}

func (is *IfStatement) TokenLiteral() string { return is.Token.Literal }
func (is *IfStatement) statementNode()       {}
func (is *IfStatement) String() string {
	out := "if " + is.Condition.String() + " " + is.Consequence.String()
	if is.Alternative != nil {
		out += " else " + is.Alternative.String()
	}
	return out
}

type MatchStatement struct {
	Token   Token
	Subject Expression
	Cases   []*MatchCase
	Default *BlockStatement
}

type MatchCase struct {
	Pattern     Expression
	Guard       Expression
	Consequence *BlockStatement
}

func (ms *MatchStatement) TokenLiteral() string { return ms.Token.Literal }
func (ms *MatchStatement) statementNode()       {}
func (ms *MatchStatement) String() string {
	var cases []string
	for _, c := range ms.Cases {
		cases = append(cases, c.Pattern.String()+" => "+c.Consequence.String())
	}
	return "match " + ms.Subject.String() + " { " + strings.Join(cases, ", ") + " }"
}

type ForStatement struct {
	Token    Token
	Item     *Identifier
	Index    *Identifier
	Iterable Expression
	Body     *BlockStatement
}

func (fs *ForStatement) TokenLiteral() string { return fs.Token.Literal }
func (fs *ForStatement) statementNode()       {}
func (fs *ForStatement) String() string {
	return "for " + fs.Item.String() + " in " + fs.Iterable.String() + " " + fs.Body.String()
}

type ForClassicStatement struct {
	Token     Token
	Init      Statement
	Condition Expression
	Update    Statement
	Body      *BlockStatement
}

func (fcs *ForClassicStatement) TokenLiteral() string { return fcs.Token.Literal }
func (fcs *ForClassicStatement) statementNode()       {}
func (fcs *ForClassicStatement) String() string {
	init := ""
	cond := ""
	update := ""
	if fcs.Init != nil {
		init = fcs.Init.String()
	}
	if fcs.Condition != nil {
		cond = fcs.Condition.String()
	}
	if fcs.Update != nil {
		update = fcs.Update.String()
	}
	return "for " + init + "; " + cond + "; " + update + " " + fcs.Body.String()
}

type ForEachStatement struct {
	Token  Token
	Key    *Identifier
	Value  *Identifier
	Object Expression
	Body   *BlockStatement
}

func (fes *ForEachStatement) TokenLiteral() string { return fes.Token.Literal }
func (fes *ForEachStatement) statementNode()       {}
func (fes *ForEachStatement) String() string {
	return "for " + fes.Key.String() + ", " + fes.Value.String() + " in " + fes.Object.String() + " " + fes.Body.String()
}

type WhileStatement struct {
	Token     Token
	Condition Expression
	Body      *BlockStatement
}

func (ws *WhileStatement) TokenLiteral() string { return ws.Token.Literal }
func (ws *WhileStatement) statementNode()       {}
func (ws *WhileStatement) String() string {
	return "while " + ws.Condition.String() + " " + ws.Body.String()
}

type DoWhileStatement struct {
	Token     Token
	Body      *BlockStatement
	Condition Expression
}

func (dws *DoWhileStatement) TokenLiteral() string { return dws.Token.Literal }
func (dws *DoWhileStatement) statementNode()       {}
func (dws *DoWhileStatement) String() string {
	return "do " + dws.Body.String() + " while " + dws.Condition.String()
}

type TryStatement struct {
	Token    Token
	TryBlock *BlockStatement
	Catches  []*CatchClause
	Finally  *BlockStatement
}

type CatchClause struct {
	Token    Token
	Variable *Identifier
	Type     *TypeExpression
	Body     *BlockStatement
}

func (ts *TryStatement) TokenLiteral() string { return ts.Token.Literal }
func (ts *TryStatement) statementNode()       {}
func (ts *TryStatement) String() string {
	out := "try " + ts.TryBlock.String()
	for _, c := range ts.Catches {
		if c.Variable != nil {
			out += " catch " + c.Variable.String() + " " + c.Body.String()
		} else {
			out += " catch " + c.Body.String()
		}
	}
	if ts.Finally != nil {
		out += " finally " + ts.Finally.String()
	}
	return out
}

type ThrowStatement struct {
	Token Token
	Value Expression
}

func (ths *ThrowStatement) TokenLiteral() string { return ths.Token.Literal }
func (ths *ThrowStatement) statementNode()       {}
func (ths *ThrowStatement) String() string {
	return "throw " + ths.Value.String()
}

type ReturnStatement struct {
	Token Token
	Value Expression
}

func (rs *ReturnStatement) TokenLiteral() string { return rs.Token.Literal }
func (rs *ReturnStatement) statementNode()       {}
func (rs *ReturnStatement) String() string {
	if rs.Value != nil {
		return "return " + rs.Value.String()
	}
	return "return"
}

type BreakStatement struct {
	Token Token
	Label string
}

func (bs *BreakStatement) TokenLiteral() string { return bs.Token.Literal }
func (bs *BreakStatement) statementNode()       {}
func (bs *BreakStatement) String() string {
	if bs.Label != "" {
		return "break " + bs.Label
	}
	return "break"
}

type ContinueStatement struct {
	Token Token
	Label string
}

func (cs *ContinueStatement) TokenLiteral() string { return cs.Token.Literal }
func (cs *ContinueStatement) statementNode()       {}
func (cs *ContinueStatement) String() string {
	if cs.Label != "" {
		return "continue " + cs.Label
	}
	return "continue"
}

type FunctionParameter struct {
	Name       *Identifier
	Type       *TypeExpression
	Default    Expression
	IsVariadic bool
	IsRef      bool
}

type FunctionStatement struct {
	Token       Token
	Name        *Identifier
	TypeParams  []string
	Parameters  []*FunctionParameter
	ReturnType  *TypeExpression
	Body        *BlockStatement
	IsAsync     bool
	IsGenerator bool
	Decorators  []*Decorator
}

func (fs *FunctionStatement) TokenLiteral() string { return fs.Token.Literal }
func (fs *FunctionStatement) statementNode()       {}
func (fs *FunctionStatement) String() string {
	var params []string
	for _, p := range fs.Parameters {
		var pstr string
		if p.IsRef {
			pstr = "ref "
		}
		pstr += p.Name.String()
		if p.Type != nil {
			pstr += ": " + p.Type.String()
		}
		if p.Default != nil {
			pstr += " = " + p.Default.String()
		}
		if p.IsVariadic {
			pstr = "..." + pstr
		}
		params = append(params, pstr)
	}
	ret := ""
	if fs.ReturnType != nil {
		ret = " -> " + fs.ReturnType.String()
	}
	async := ""
	if fs.IsAsync {
		async = "async "
	}
	return async + "fn " + fs.Name.String() + "(" + strings.Join(params, ", ") + ")" + ret + " " + fs.Body.String()
}

type ClassStatement struct {
	Token        Token
	Name         *Identifier
	TypeParams   []string
	Extends      *Identifier
	Implements   []*Identifier
	Mixins       []*Identifier
	Properties   []*PropertyStatement
	Methods      []*FunctionStatement
	Constructors []*FunctionStatement
	StaticBlocks []*BlockStatement
	Decorators   []*Decorator
}

type PropertyStatement struct {
	Token  Token
	Name   *Identifier
	Type   *TypeExpression
	Value  Expression
	Mut    bool
	Static bool
}

func (ps *PropertyStatement) TokenLiteral() string { return ps.Token.Literal }
func (ps *PropertyStatement) statementNode()       {}
func (ps *PropertyStatement) String() string {
	val := ""
	if ps.Value != nil {
		val = " = " + ps.Value.String()
	}
	return ps.Name.String() + val
}

func (cs *ClassStatement) TokenLiteral() string { return cs.Token.Literal }
func (cs *ClassStatement) statementNode()       {}
func (cs *ClassStatement) String() string {
	ext := ""
	if cs.Extends != nil {
		ext = " extends " + cs.Extends.String()
	}
	var props []string
	for _, p := range cs.Properties {
		props = append(props, p.String())
	}
	for _, m := range cs.Methods {
		props = append(props, m.String())
	}
	return "class " + cs.Name.String() + ext + " { " + strings.Join(props, "; ") + " }"
}

type Decorator struct {
	Token     Token
	Name      *Identifier
	Arguments []Expression
}

func (d *Decorator) TokenLiteral() string { return d.Token.Literal }
func (d *Decorator) String() string {
	if d.Arguments != nil {
		var args []string
		for _, a := range d.Arguments {
			args = append(args, a.String())
		}
		return "@" + d.Name.String() + "(" + strings.Join(args, ", ") + ")"
	}
	return "@" + d.Name.String()
}

type InterfaceStatement struct {
	Token      Token
	Name       *Identifier
	TypeParams []string
	Extends    []*Identifier
	Methods    []*FunctionSignature
}

type FunctionSignature struct {
	Name       *Identifier
	Parameters []*FunctionParameter
	ReturnType *TypeExpression
}

func (is *InterfaceStatement) TokenLiteral() string { return is.Token.Literal }
func (is *InterfaceStatement) statementNode()       {}
func (is *InterfaceStatement) String() string {
	return "interface " + is.Name.String() + " { }"
}

type EnumStatement struct {
	Token      Token
	Name       *Identifier
	Implements []*Identifier
	Cases      []*EnumCase
	Raw        bool
}

type EnumCase struct {
	Token      Token
	Name       *Identifier
	Associated []Expression
	RawValue   Expression
}

func (es *EnumStatement) TokenLiteral() string { return es.Token.Literal }
func (es *EnumStatement) statementNode()       {}
func (es *EnumStatement) String() string {
	return "enum " + es.Name.String() + " { }"
}

type StructStatement struct {
	Token        Token
	Name         *Identifier
	TypeParams   []string
	Fields       []*PropertyStatement
	Methods      []*FunctionStatement
	Constructors []*FunctionStatement
}

func (ss *StructStatement) TokenLiteral() string { return ss.Token.Literal }
func (ss *StructStatement) statementNode()       {}
func (ss *StructStatement) String() string {
	return "struct " + ss.Name.String() + " { }"
}

type TraitStatement struct {
	Token   Token
	Name    *Identifier
	Methods []*FunctionSignature
}

func (ts *TraitStatement) TokenLiteral() string { return ts.Token.Literal }
func (ts *TraitStatement) statementNode()       {}
func (ts *TraitStatement) String() string {
	return "trait " + ts.Name.String() + " { }"
}

type ImplStatement struct {
	Token   Token
	Type    *TypeExpression
	Traits  []*Identifier
	Methods []*FunctionStatement
}

func (is *ImplStatement) TokenLiteral() string { return is.Token.Literal }
func (is *ImplStatement) statementNode()       {}
func (is *ImplStatement) String() string {
	return "impl " + is.Type.String() + " { }"
}

type TypeAliasStatement struct {
	Token Token
	Name  *Identifier
	Type  *TypeExpression
}

func (tas *TypeAliasStatement) TokenLiteral() string { return tas.Token.Literal }
func (tas *TypeAliasStatement) statementNode()       {}
func (tas *TypeAliasStatement) String() string {
	return "type " + tas.Name.String() + " = " + tas.Type.String()
}

type ImportStatement struct {
	Token      Token
	Module     *StringLiteral
	Alias      *Identifier
	Items      []*Identifier
	IsDefault  bool
	IsWildcard bool
}

func (is *ImportStatement) TokenLiteral() string { return is.Token.Literal }
func (is *ImportStatement) statementNode()       {}
func (is *ImportStatement) String() string {
	items := ""
	if len(is.Items) > 0 {
		var itemStrs []string
		for _, i := range is.Items {
			itemStrs = append(itemStrs, i.String())
		}
		items = " { " + strings.Join(itemStrs, ", ") + " }"
	}
	if is.IsDefault {
		items = is.Alias.String() + items
	}
	if is.IsWildcard {
		items = "*"
	}
	return "import " + items + " from " + is.Module.String()
}

type ExportStatement struct {
	Token     Token
	Items     []Statement
	Default   Statement
	IsDefault bool
	IsAll     bool
}

func (es *ExportStatement) TokenLiteral() string { return es.Token.Literal }
func (es *ExportStatement) statementNode()       {}
func (es *ExportStatement) String() string {
	return "export { }"
}

type UsingStatement struct {
	Token      Token
	Expression Expression
	Alias      *Identifier
	Body       *BlockStatement
}

func (us *UsingStatement) TokenLiteral() string { return us.Token.Literal }
func (us *UsingStatement) statementNode()       {}
func (us *UsingStatement) String() string {
	return "using " + us.Expression.String() + " " + us.Body.String()
}

type LabelStatement struct {
	Token Token
	Name  string
}

func (ls *LabelStatement) TokenLiteral() string { return ls.Token.Literal }
func (ls *LabelStatement) statementNode()       {}
func (ls *LabelStatement) String() string {
	return ls.Name + ":"
}

type CommentStatement struct {
	Token Token
	Text  string
}

func (cs *CommentStatement) TokenLiteral() string { return cs.Token.Literal }
func (cs *CommentStatement) statementNode()       {}
func (cs *CommentStatement) String() string {
	return "// " + cs.Text
}

type EmptyStatement struct {
	Token Token
}

func (es *EmptyStatement) TokenLiteral() string { return es.Token.Literal }
func (es *EmptyStatement) statementNode()       {}
func (es *EmptyStatement) String() string       { return "" }
