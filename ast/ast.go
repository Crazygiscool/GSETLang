package ast

import "strconv"

type Token struct {
	Type    string
	Literal string
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
}

func (p *Program) TokenLiteral() string { return "" }

type CallExpression struct {
	Token     Token
	Function  string
	Arguments []Expression
}

func (ce *CallExpression) TokenLiteral() string { return ce.Token.Literal }
func (ce *CallExpression) expressionNode()      {}
func (ce *CallExpression) statementNode()       {}
func (ce *CallExpression) String() string {
	var args []string
	for _, a := range ce.Arguments {
		args = append(args, a.String())
	}
	return "Call(" + ce.Function + " [" + join(args, ", ") + "])"
}

type Identifier struct {
	Token Token
	Value string
}

func (i *Identifier) TokenLiteral() string { return i.Token.Literal }
func (i *Identifier) expressionNode()      {}
func (i *Identifier) String() string       { return "Ident(" + i.Value + ")" }

type IntegerLiteral struct {
	Token Token
	Value int64
}

func (il *IntegerLiteral) TokenLiteral() string { return il.Token.Literal }
func (il *IntegerLiteral) expressionNode()      {}
func (il *IntegerLiteral) String() string       { return "Int(" + strconv.FormatInt(il.Value, 10) + ")" }

type FloatLiteral struct {
	Token Token
	Value float64
}

func (fl *FloatLiteral) TokenLiteral() string { return fl.Token.Literal }
func (fl *FloatLiteral) expressionNode()      {}
func (fl *FloatLiteral) String() string {
	return "Float(" + strconv.FormatFloat(fl.Value, 'f', -1, 64) + ")"
}

type StringLiteral struct {
	Token Token
	Value string
}

func (sl *StringLiteral) TokenLiteral() string { return sl.Token.Literal }
func (sl *StringLiteral) expressionNode()      {}
func (sl *StringLiteral) statementNode()       {}
func (sl *StringLiteral) String() string       { return "String(" + sl.Value + ")" }

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
	return "[" + join(els, ", ") + "]"
}

type MapLiteral struct {
	Token Token
	Pairs map[Expression]Expression
}

func (ml *MapLiteral) TokenLiteral() string { return ml.Token.Literal }
func (ml *MapLiteral) expressionNode()      {}
func (ml *MapLiteral) String() string       { return "Map{}" }

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

type BlockStatement struct {
	Token      Token
	Statements []Statement
}

func (bs *BlockStatement) TokenLiteral() string { return bs.Token.Literal }
func (bs *BlockStatement) statementNode()       {}
func (bs *BlockStatement) String() string {
	var out string
	for _, s := range bs.Statements {
		out += s.String() + ";"
	}
	return "{ " + out + " }"
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
	Name  string
	Value Expression
}

func (vs *VariableStatement) TokenLiteral() string { return vs.Token.Literal }
func (vs *VariableStatement) statementNode()       {}
func (vs *VariableStatement) String() string       { return "var " + vs.Name + " = " + vs.Value.String() }

type AssignmentStatement struct {
	Token    Token
	Name     string
	Operator string
	Value    Expression
}

func (as *AssignmentStatement) TokenLiteral() string { return as.Token.Literal }
func (as *AssignmentStatement) statementNode()       {}
func (as *AssignmentStatement) String() string {
	return as.Name + " " + as.Operator + " " + as.Value.String()
}

type IfStatement struct {
	Token       Token
	Condition   Expression
	Consequence *BlockStatement
	Alternative *BlockStatement
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

type ForStatement struct {
	Token     Token
	Init      *VariableStatement
	Condition Expression
	Update    *AssignmentStatement
	Body      *BlockStatement
}

func (fs *ForStatement) TokenLiteral() string { return fs.Token.Literal }
func (fs *ForStatement) statementNode()       {}
func (fs *ForStatement) String() string {
	return "for " + fs.Init.String() + "; " + fs.Condition.String() + "; " + fs.Update.String() + " " + fs.Body.String()
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

type FunctionStatement struct {
	Token      Token
	Name       string
	Parameters []string
	Body       *BlockStatement
}

func (fs *FunctionStatement) TokenLiteral() string { return fs.Token.Literal }
func (fs *FunctionStatement) statementNode()       {}
func (fs *FunctionStatement) String() string {
	var params []string
	for _, p := range fs.Parameters {
		params = append(params, p)
	}
	return "fn " + fs.Name + "(" + join(params, ", ") + ") " + fs.Body.String()
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
}

func (bs *BreakStatement) TokenLiteral() string { return bs.Token.Literal }
func (bs *BreakStatement) statementNode()       {}
func (bs *BreakStatement) String() string       { return "break" }

type ContinueStatement struct {
	Token Token
}

func (cs *ContinueStatement) TokenLiteral() string { return cs.Token.Literal }
func (cs *ContinueStatement) statementNode()       {}
func (cs *ContinueStatement) String() string       { return "continue" }

func join(ss []string, sep string) string {
	if len(ss) == 0 {
		return ""
	}
	out := ss[0]
	for i := 1; i < len(ss); i++ {
		out += sep + ss[i]
	}
	return out
}
