package main

// Every node in our tree must implement this
type Node interface {
	TokenLiteral() string
}

// Expression represents a piece of code that returns a value
type Expression interface {
	Node
	expressionNode()
	String() string
}

// Statement represents an instruction (like a function call)
type Statement interface {
	Node
	statementNode()
	String() string
}

// CallExpression represents: function(args)
type CallExpression struct {
	Token     Token        // The identifier (e.g., shout)
	Function  string       // The name of the function
	Arguments []Expression // The stuff inside the brackets
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

type StringLiteral struct {
	Token Token
	Value string
}

func (sl *StringLiteral) TokenLiteral() string { return sl.Token.Literal }
func (sl *StringLiteral) expressionNode()      {}
func (sl *StringLiteral) statementNode()       {}
func (sl *StringLiteral) String() string {
	return "String(" + sl.Value + ")"
}

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
