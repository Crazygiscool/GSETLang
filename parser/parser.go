package parser

import (
	"fmt"
	"gsetlang/ast"
	"gsetlang/lexer"
	"strconv"
	"strings"
)

type Parser struct {
	l       *lexer.Lexer
	curTok  ast.Token
	peekTok ast.Token
	errors  []string
	prog    *ast.Program
}

func New(l *lexer.Lexer) *Parser {
	p := &Parser{l: l, errors: []string{}, prog: &ast.Program{Statements: []ast.Statement{}, Imports: []*ast.ImportStatement{}}}
	p.peekTok = l.NextToken()
	p.nextToken()
	return p
}

func (p *Parser) nextToken() {
	p.curTok = p.peekTok
	p.peekTok = p.l.NextToken()
}

func (p *Parser) Errors() []string { return p.errors }

func (p *Parser) peekError(t string) {
	p.errors = append(p.errors, fmt.Sprintf("line %d: expected %s, got %s", p.curTok.Line, t, p.peekTok.Type))
}

func (p *Parser) ParseProgram() *ast.Program {
	for p.curTok.Type != "EOF" {
		for p.curTok.Type == "NEWLINE" || p.curTok.Type == "SEMICOLON" {
			p.nextToken()
		}
		if p.curTok.Type == "RBRACE" || p.curTok.Type == "EOF" {
			if p.curTok.Type == "RBRACE" {
				p.nextToken()
			}
			continue
		}
		if p.curTok.Type == "IMPORT" {
			if imp := p.parseImportStatement(); imp != nil {
				p.prog.Imports = append(p.prog.Imports, imp)
				p.nextToken()
			}
		} else if stmt := p.parseStatement(); stmt != nil {
			if fn, ok := stmt.(*ast.FunctionStatement); ok {
				p.prog.Functions = append(p.prog.Functions, fn)
			} else if cls, ok := stmt.(*ast.ClassStatement); ok {
				p.prog.Classes = append(p.prog.Classes, cls)
			}
			p.prog.Statements = append(p.prog.Statements, stmt)
			if p.curTok.Type != "EOF" && p.curTok.Type != "RBRACE" {
				p.nextToken()
			}
		}
	}
	return p.prog
}

func (p *Parser) parseStatement() ast.Statement {
	if p.curTok.Type == "RBRACE" || p.curTok.Type == "EOF" {
		return nil
	}
	switch p.curTok.Type {
	case "VAR", "VAL", "LET", "CONST":
		return p.parseVariableStatement()
	case "IF":
		return p.parseIfStatement()
	case "MATCH":
		return p.parseMatchStatement()
	case "FOR", "FOREACH":
		return p.parseForStatement()
	case "WHILE":
		return p.parseWhileStatement()
	case "DO":
		return p.parseDoWhileStatement()
	case "FUNCTION", "ASYNC":
		return p.parseFunctionStatement()
	case "CLASS", "STRUCT", "ENUM", "TRAIT", "INTERFACE":
		return p.parseClassStatement()
	case "RETURN":
		return p.parseReturnStatement()
	case "THROW":
		return p.parseThrowStatement()
	case "TRY":
		return p.parseTryStatement()
	case "BREAK":
		return p.parseBreakStatement()
	case "CONTINUE":
		return p.parseContinueStatement()
	case "IMPORT":
		return p.parseImportStatement()
	case "EXPORT":
		return p.parseExportStatement()
	case "TYPE":
		return p.parseTypeAliasStatement()
	case "IMPLEMENTS", "EXTENDS", "MIXIN":
		return nil
	default:
		return p.parseExpressionOrAssignment()
	}
}

// Import Statement
func (p *Parser) parseImportStatement() *ast.ImportStatement {
	if p.curTok.Type != "IMPORT" {
		return nil
	}
	p.nextToken()

	imp := &ast.ImportStatement{Token: p.curTok}

	switch p.curTok.Type {
	case "STRING":
		imp.Module = &ast.StringLiteral{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
	case "IDENT":
		moduleName := p.curTok.Literal
		p.nextToken()
		imp.Module = &ast.StringLiteral{Token: p.curTok, Value: moduleName}
	}

	if p.curTok.Type == "AS" {
		p.nextToken()
		imp.Alias = &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
	}

	if p.curTok.Type == "COMMA" || p.peekTok.Type == "LBRACE" {
		imp.IsDefault = true
		if p.curTok.Type == "COMMA" {
			p.nextToken()
		}
	}

	if p.curTok.Type == "LBRACE" {
		p.nextToken()
		for p.curTok.Type != "RBRACE" && p.curTok.Type != "EOF" {
			item := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
			p.prog.Imports = append(p.prog.Imports, &ast.ImportStatement{Token: p.curTok, Module: imp.Module, Items: []*ast.Identifier{item}})
			p.nextToken()
			if p.curTok.Type == "COMMA" {
				p.nextToken()
			}
		}
		if p.curTok.Type == "RBRACE" {
			p.nextToken()
		}
		return nil
	}

	if p.curTok.Type == "FROM" {
		p.nextToken()
		if p.curTok.Type == "STRING" {
			imp.Module = &ast.StringLiteral{Token: p.curTok, Value: p.curTok.Literal}
			p.nextToken()
		}
	}

	return imp
}

func (p *Parser) parseExportStatement() *ast.ExportStatement {
	p.nextToken()
	exp := &ast.ExportStatement{Token: p.curTok}
	if p.curTok.Type == "LBRACE" {
		p.nextToken()
		for p.curTok.Type != "RBRACE" && p.curTok.Type != "EOF" {
			if fn := p.parseFunctionStatement(); fn != nil {
				exp.Items = append(exp.Items, fn)
			}
			if p.curTok.Type == "COMMA" {
				p.nextToken()
			}
		}
		if p.curTok.Type == "RBRACE" {
			p.nextToken()
		}
	}
	return exp
}

func (p *Parser) parseTypeAliasStatement() *ast.TypeAliasStatement {
	if p.curTok.Type != "TYPE" {
		return nil
	}
	p.nextToken()

	name := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
	p.nextToken()

	var typeExpr *ast.TypeExpression
	if p.curTok.Type == "LT" {
		typeExpr = p.parseTypeExpression()
	} else {
		typeExpr = &ast.TypeExpression{Token: p.curTok, Name: p.curTok.Literal}
		p.nextToken()
	}

	return &ast.TypeAliasStatement{Token: p.curTok, Name: name, Type: typeExpr}
}

// Variable Statement: var/val/let/const x = expr
func (p *Parser) parseVariableStatement() ast.Statement {
	tok := p.curTok
	isMut := p.curTok.Type == "VAR" || p.curTok.Type == "MUT"
	p.nextToken()

	name := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
	p.nextToken()

	var typ *ast.TypeExpression
	if p.curTok.Type == "COLON" {
		p.nextToken()
		typ = p.parseTypeExpression()
	}

	if p.curTok.Type != "ASSIGN" && p.curTok.Type != "COLON_ASSIGN" {
		return &ast.VariableStatement{Token: tok, Name: name, Type: typ, Value: &ast.NilLiteral{Token: tok}, Mut: isMut}
	}

	op := p.curTok.Literal
	p.nextToken()

	val := p.parseExpression()

	return &ast.AssignmentStatement{Token: tok, Name: name, Operator: op, Value: val}
}

// If Statement
func (p *Parser) parseIfStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	condition := p.parseExpression()
	consequence := p.parseBlock()

	for p.curTok.Type == "NEWLINE" {
		p.nextToken()
	}
	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	var alternative ast.Statement
	if p.curTok.Type == "ELSE" {
		p.nextToken()
		if p.curTok.Type == "IF" {
			alternative = p.parseIfStatement()
		} else {
			alternative = p.parseBlock()
		}
	}

	return &ast.IfStatement{Token: tok, Condition: condition, Consequence: consequence, Alternative: alternative}
}

// Match Statement (pattern matching)
func (p *Parser) parseMatchStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	subject := p.parseExpression()

	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return nil
	}
	p.nextToken()

	cases := []*ast.MatchCase{}
	for p.curTok.Type != "RBRACE" && p.curTok.Type != "EOF" {
		pattern := p.parseExpression()

		var guard ast.Expression
		if p.curTok.Type == "IF" {
			p.nextToken()
			guard = p.parseExpression()
		}

		if p.curTok.Type == "ARROW" || p.curTok.Type == "FAT_ARROW" {
			p.nextToken()
		}

		body := p.parseBlock()
		cases = append(cases, &ast.MatchCase{Pattern: pattern, Guard: guard, Consequence: body})

		if p.curTok.Type == "COMMA" {
			p.nextToken()
		}
	}

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.MatchStatement{Token: tok, Subject: subject, Cases: cases}
}

// For Statement
func (p *Parser) parseForStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	if p.curTok.Type == "LPAREN" || p.peekTok.Type == "IN" || p.peekTok.Type == "COMMA" {
		return p.parseForInStatement(tok)
	}

	return p.parseForClassicStatement(tok)
}

func (p *Parser) parseForInStatement(tok ast.Token) ast.Statement {
	if p.curTok.Type == "LPAREN" {
		p.nextToken()
	}

	item := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
	p.nextToken()

	var index *ast.Identifier
	if p.curTok.Type == "COMMA" {
		p.nextToken()
		index = &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
	}

	if p.curTok.Type != "IN" {
		p.peekError("IN")
		return nil
	}
	p.nextToken()

	iterable := p.parseExpression()

	if p.curTok.Type == "RPAREN" {
		p.nextToken()
	}

	body := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	if index != nil {
		return &ast.ForEachStatement{Token: tok, Key: item, Value: index, Object: iterable, Body: body}
	}
	return &ast.ForStatement{Token: tok, Item: item, Iterable: iterable, Body: body}
}

func (p *Parser) parseForClassicStatement(tok ast.Token) *ast.ForClassicStatement {
	var init ast.Statement
	var condition ast.Expression
	var update ast.Statement

	if p.curTok.Type != "SEMICOLON" {
		if p.curTok.Type == "VAR" || p.curTok.Type == "VAL" || p.curTok.Type == "LET" {
			init = p.parseVariableStatement()
		} else {
			init = p.parseExpressionStatement()
		}
	}

	if p.curTok.Type == "SEMICOLON" {
		p.nextToken()
		condition = p.parseExpression()
	}

	if p.curTok.Type == "SEMICOLON" {
		p.nextToken()
		update = p.parseExpressionStatement()
	}

	body := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.ForClassicStatement{Token: tok, Init: init, Condition: condition, Update: update, Body: body}
}

// While Statement
func (p *Parser) parseWhileStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	condition := p.parseExpression()
	body := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.WhileStatement{Token: tok, Condition: condition, Body: body}
}

// Do-While Statement
func (p *Parser) parseDoWhileStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	body := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	if p.curTok.Type == "WHILE" {
		p.nextToken()
		condition := p.parseExpression()
		return &ast.DoWhileStatement{Token: tok, Body: body, Condition: condition}
	}

	return &ast.DoWhileStatement{Token: tok, Body: body, Condition: &ast.BooleanLiteral{Token: tok, Value: true}}
}

// Function Statement
func (p *Parser) parseFunctionStatement() ast.Statement {
	tok := p.curTok
	isAsync := false

	if p.curTok.Type == "ASYNC" {
		isAsync = true
		p.nextToken()
	}

	if p.curTok.Type != "FUNCTION" && p.curTok.Type != "IDENT" {
		p.peekError("FUNCTION")
		return nil
	}
	if p.curTok.Type == "FUNCTION" {
		p.nextToken()
	}

	name := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
	p.nextToken()

	if p.curTok.Type != "LPAREN" {
		p.peekError("LPAREN")
		return nil
	}

	params := p.parseFunctionParams()

	var returnType *ast.TypeExpression
	if p.curTok.Type == "RETURN_ARROW" || p.curTok.Type == "COLON" {
		if p.curTok.Type == "RETURN_ARROW" {
			p.nextToken()
		} else {
			p.nextToken()
			returnType = p.parseTypeExpression()
		}
	}

	body := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.FunctionStatement{Token: tok, Name: name, Parameters: params, ReturnType: returnType, Body: body, IsAsync: isAsync}
}

func (p *Parser) parseFunctionParams() []*ast.FunctionParameter {
	p.nextToken()
	params := []*ast.FunctionParameter{}

	if p.curTok.Type == "RPAREN" {
		p.nextToken()
		return params
	}

	for {
		param := &ast.FunctionParameter{}

		if p.curTok.Type == "ELLIPSIS" || p.peekTok.Type == "DOT" {
			param.IsVariadic = true
			p.nextToken()
		}

		param.Name = &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()

		if p.curTok.Type == "COLON" {
			p.nextToken()
			param.Type = p.parseTypeExpression()
		}

		if p.curTok.Type == "ASSIGN" {
			p.nextToken()
			param.Default = p.parseExpression()
		}

		params = append(params, param)

		if p.curTok.Type == "COMMA" {
			p.nextToken()
			continue
		}
		break
	}

	if p.curTok.Type == "RPAREN" {
		p.nextToken()
	}

	return params
}

// Class Statement
func (p *Parser) parseClassStatement() ast.Statement {
	tok := p.curTok
	classType := p.curTok.Type
	p.nextToken()

	name := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
	p.nextToken()

	var extends *ast.Identifier
	if p.curTok.Type == "EXTENDS" {
		p.nextToken()
		extends = &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
	}

	var implements []*ast.Identifier
	if p.curTok.Type == "IMPLEMENTS" {
		p.nextToken()
		for p.curTok.Type == "IDENT" {
			implements = append(implements, &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal})
			p.nextToken()
			if p.curTok.Type != "COMMA" {
				break
			}
			p.nextToken()
		}
	}

	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return nil
	}

	_ = p.parseBlock()

	if classType == "ENUM" {
		return &ast.EnumStatement{Token: tok, Name: name, Implements: implements}
	}
	if classType == "INTERFACE" {
		return &ast.InterfaceStatement{Token: tok, Name: name}
	}
	if classType == "TRAIT" {
		return &ast.TraitStatement{Token: tok, Name: name}
	}
	if classType == "STRUCT" {
		return &ast.StructStatement{Token: tok, Name: name}
	}

	return &ast.ClassStatement{Token: tok, Name: name, Extends: extends, Implements: implements}
}

// Return Statement
func (p *Parser) parseReturnStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	if p.curTok.Type == "SEMICOLON" || p.curTok.Type == "RBRACE" || p.curTok.Type == "EOF" {
		return &ast.ReturnStatement{Token: tok}
	}

	val := p.parseExpression()
	return &ast.ReturnStatement{Token: tok, Value: val}
}

// Throw Statement
func (p *Parser) parseThrowStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()
	val := p.parseExpression()
	return &ast.ThrowStatement{Token: tok, Value: val}
}

// Try Statement
func (p *Parser) parseTryStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()

	tryBlock := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	catches := []*ast.CatchClause{}
	for p.curTok.Type == "CATCH" {
		p.nextToken()

		var variable *ast.Identifier
		var catchType *ast.TypeExpression

		if p.curTok.Type == "IDENT" && p.peekTok.Type != "LBRACE" {
			variable = &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
			p.nextToken()
			if p.curTok.Type == "COLON" {
				p.nextToken()
				catchType = p.parseTypeExpression()
			}
		}

		body := p.parseBlock()
		if p.curTok.Type == "RBRACE" {
			p.nextToken()
		}
		catches = append(catches, &ast.CatchClause{Token: tok, Variable: variable, Type: catchType, Body: body})
	}

	var finally *ast.BlockStatement
	if p.curTok.Type == "FINALLY" {
		p.nextToken()
		finally = p.parseBlock()
		if p.curTok.Type == "RBRACE" {
			p.nextToken()
		}
	}

	return &ast.TryStatement{Token: tok, TryBlock: tryBlock, Catches: catches, Finally: finally}
}

// Break Statement
func (p *Parser) parseBreakStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()
	var label string
	if p.curTok.Type == "IDENT" {
		label = p.curTok.Literal
		p.nextToken()
	}
	return &ast.BreakStatement{Token: tok, Label: label}
}

// Continue Statement
func (p *Parser) parseContinueStatement() ast.Statement {
	tok := p.curTok
	p.nextToken()
	var label string
	if p.curTok.Type == "IDENT" {
		label = p.curTok.Literal
		p.nextToken()
	}
	return &ast.ContinueStatement{Token: tok, Label: label}
}

// Block Statement
func (p *Parser) parseBlock() *ast.BlockStatement {
	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return &ast.BlockStatement{Token: p.curTok}
	}
	p.nextToken()

	block := &ast.BlockStatement{Token: p.curTok, Statements: []ast.Statement{}}

	for {
		for p.curTok.Type == "NEWLINE" || p.curTok.Type == "SEMICOLON" {
			p.nextToken()
		}
		if p.curTok.Type == "RBRACE" || p.curTok.Type == "EOF" {
			break
		}
		if stmt := p.parseStatement(); stmt != nil {
			block.Statements = append(block.Statements, stmt)
		}
		for p.curTok.Type == "NEWLINE" || p.curTok.Type == "SEMICOLON" {
			p.nextToken()
		}
		if p.curTok.Type == "RBRACE" || p.curTok.Type == "EOF" {
			break
		}
	}

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return block
}

// Expression Statement
func (p *Parser) parseExpressionOrAssignment() ast.Statement {
	expr := p.parseExpression()

	if p.curTok.Type == "ASSIGN" || p.curTok.Type == "PLUS_EQ" ||
		p.curTok.Type == "MINUS_EQ" || p.curTok.Type == "MUL_EQ" ||
		p.curTok.Type == "DIV_EQ" || p.curTok.Type == "COLON_ASSIGN" {

		op := p.curTok.Literal
		p.nextToken()
		val := p.parseExpression()
		return &ast.AssignmentStatement{Token: p.curTok, Name: expr, Operator: op, Value: val}
	}

	return &ast.ExpressionStatement{Token: p.curTok, Expression: expr}
}

func (p *Parser) parseExpressionStatement() ast.Statement {
	expr := p.parseExpression()
	return &ast.ExpressionStatement{Token: p.curTok, Expression: expr}
}

// Expression Parsing
func (p *Parser) parseExpression() ast.Expression {
	return p.parseAssignment()
}

func (p *Parser) parseAssignment() ast.Expression {
	left := p.parseTernary()

	if p.curTok.Type == "ASSIGN" || p.curTok.Type == "PLUS_EQ" ||
		p.curTok.Type == "MINUS_EQ" || p.curTok.Type == "MUL_EQ" ||
		p.curTok.Type == "DIV_EQ" || p.curTok.Type == "COLON_ASSIGN" {

		op := p.curTok.Literal
		p.nextToken()
		val := p.parseAssignment()
		return &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: val}
	}

	return left
}

func (p *Parser) parseTernary() ast.Expression {
	cond := p.parseOr()

	if p.curTok.Type == "QUESTION" {
		p.nextToken()
		cons := p.parseAssignment()
		if p.curTok.Type == "COLON" {
			p.nextToken()
			alt := p.parseAssignment()
			return &ast.TernaryExpression{Token: p.curTok, Condition: cond, Consequence: cons, Alternative: alt}
		}
	}

	return cond
}

func (p *Parser) parseOr() ast.Expression {
	left := p.parseAnd()

	for p.curTok.Type == "OR" || p.curTok.Type == "NULL_COALESCE" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseAnd()
		if op == "??" {
			left = &ast.NullCoalescingExpression{Token: p.curTok, Left: left, Right: right}
		} else {
			left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
		}
	}

	return left
}

func (p *Parser) parseAnd() ast.Expression {
	left := p.parseEquality()

	for p.curTok.Type == "AND" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseEquality()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseEquality() ast.Expression {
	left := p.parseComparison()

	for p.curTok.Type == "EQ" || p.curTok.Type == "NEQ" ||
		p.curTok.Type == "STRICT_EQ" || p.curTok.Type == "STRICT_NEQ" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseComparison()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseComparison() ast.Expression {
	left := p.parseBitwiseOr()

	for p.curTok.Type == "LT" || p.curTok.Type == "GT" ||
		p.curTok.Type == "LTE" || p.curTok.Type == "GTE" ||
		p.curTok.Type == "SPACESHIP" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseBitwiseOr()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseBitwiseOr() ast.Expression {
	left := p.parseBitwiseXor()

	for p.curTok.Type == "BITOR" || p.curTok.Type == "OR" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseBitwiseXor()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseBitwiseXor() ast.Expression {
	left := p.parseBitwiseAnd()

	for p.curTok.Type == "XOR" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseBitwiseAnd()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseBitwiseAnd() ast.Expression {
	left := p.parseShift()

	for p.curTok.Type == "BITAND" || p.curTok.Type == "AND" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseShift()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseShift() ast.Expression {
	left := p.parseAdditive()

	for p.curTok.Type == "LSHIFT" || p.curTok.Type == "RSHIFT" ||
		p.curTok.Type == "RSHIFT_UNSIGNED" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseAdditive()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseAdditive() ast.Expression {
	left := p.parseMultiplicative()

	for p.curTok.Type == "PLUS" || p.curTok.Type == "MINUS" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseMultiplicative()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseMultiplicative() ast.Expression {
	left := p.parsePower()

	for p.curTok.Type == "ASTERISK" || p.curTok.Type == "SLASH" ||
		p.curTok.Type == "MOD" || p.curTok.Type == "PERCENT" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parsePower()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parsePower() ast.Expression {
	left := p.parseUnary()

	if p.curTok.Type == "POWER" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseUnary()
		left = &ast.InfixExpression{Token: p.curTok, Left: left, Operator: op, Right: right}
	}

	return left
}

func (p *Parser) parseUnary() ast.Expression {
	if p.curTok.Type == "BANG" || p.curTok.Type == "MINUS" ||
		p.curTok.Type == "PLUS_PLUS" || p.curTok.Type == "MINUS_MINUS" ||
		p.curTok.Type == "NOT" || p.curTok.Type == "TYPEOF" {
		op := p.curTok.Literal
		p.nextToken()
		right := p.parseUnary()
		return &ast.PrefixExpression{Token: p.curTok, Operator: op, Right: right}
	}

	if p.curTok.Type == "AWAIT" {
		tok := p.curTok
		p.nextToken()
		val := p.parseUnary()
		return &ast.AwaitExpression{Token: tok, Value: val}
	}

	if p.curTok.Type == "YIELD" {
		tok := p.curTok
		p.nextToken()
		var val ast.Expression
		if p.curTok.Type != "SEMICOLON" && p.curTok.Type != "COMMA" {
			val = p.parseUnary()
		}
		return &ast.YieldExpression{Token: tok, Value: val}
	}

	return p.parsePostfix()
}

func (p *Parser) parsePostfix() ast.Expression {
	expr := p.parsePrimary()

	for p.curTok.Type == "PLUS_PLUS" || p.curTok.Type == "MINUS_MINUS" ||
		p.curTok.Type == "QUESTION_DOT" || p.curTok.Type == "DOT" {
		if p.curTok.Type == "PLUS_PLUS" || p.curTok.Type == "MINUS_MINUS" {
			op := p.curTok.Literal
			p.nextToken()
			expr = &ast.InfixExpression{Token: p.curTok, Left: expr, Operator: op, Right: &ast.IntegerLiteral{Token: p.curTok, Value: 1}}
		} else if p.curTok.Type == "DOT" || p.curTok.Type == "QUESTION_DOT" {
			_ = p.curTok.Type == "QUESTION_DOT"
			p.nextToken()
			if p.curTok.Type == "IDENT" {
				method := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
				p.nextToken()
				if p.curTok.Type == "LPAREN" {
					args := p.parseCallArguments()
					expr = &ast.MethodCallExpression{Token: p.curTok, Object: expr, Method: method, Arguments: args}
				} else {
					expr = &ast.MemberExpression{Token: p.curTok, Object: expr, Property: method, Computed: false}
				}
			}
		}
	}

	return expr
}

func (p *Parser) parsePrimary() ast.Expression {
	switch p.curTok.Type {
	case "IDENT":
		ident := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()

		if p.curTok.Type == "LPAREN" {
			args := p.parseCallArguments()
			return &ast.CallExpression{Token: p.curTok, Function: ident, Arguments: args}
		}
		if p.curTok.Type == "ARROW" {
			p.nextToken()
			body := p.parseBlock()
			return &ast.LambdaExpression{Token: p.curTok, Parameters: []*ast.FunctionParameter{{Name: ident}}, Body: body}
		}
		if p.curTok.Type == "QUESTION" {
			p.nextToken()
			if p.curTok.Type == "LPAREN" {
				args := p.parseCallArguments()
				return &ast.CallExpression{Token: p.curTok, Function: &ast.Identifier{Token: p.curTok, Value: "safeCall_" + ident.Value}, Arguments: args}
			}
		}
		return ident

	case "INT":
		val, _ := strconv.ParseInt(p.curTok.Literal, 10, 64)
		p.nextToken()
		return &ast.IntegerLiteral{Token: p.curTok, Value: val}

	case "FLOAT":
		val, _ := strconv.ParseFloat(p.curTok.Literal, 64)
		p.nextToken()
		return &ast.FloatLiteral{Token: p.curTok, Value: val}

	case "STRING", "RAW_STRING":
		lit := &ast.StringLiteral{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
		return lit

	case "TEMPLATE_STRING":
		lit := &ast.StringLiteral{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
		return lit

	case "CHAR":
		lit := &ast.StringLiteral{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
		return lit

	case "TRUE", "FALSE":
		val := p.curTok.Type == "TRUE"
		p.nextToken()
		return &ast.BooleanLiteral{Token: p.curTok, Value: val}

	case "NIL", "NULL", "NONE", "UNDEFINED":
		p.nextToken()
		return &ast.NilLiteral{Token: p.curTok}

	case "LBRACKET":
		return p.parseArrayOrComprehension()

	case "HASH_BRACE":
		return p.parseSetLiteral()

	case "LBRACE":
		return p.parseMapOrBlock()

	case "LPAREN":
		p.nextToken()
		expr := p.parseExpression()
		if p.curTok.Type == "RPAREN" {
			p.nextToken()
		}
		return expr

	case "FUNCTION", "ASYNC":
		return p.parseLambdaOrFunction()

	case "NEW":
		tok := p.curTok
		p.nextToken()
		if p.curTok.Type == "IDENT" {
			cls := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
			p.nextToken()
			var args []ast.Expression
			if p.curTok.Type == "LPAREN" {
				args = p.parseCallArguments()
			}
			return &ast.CallExpression{Token: tok, Function: cls, Arguments: args}
		}

	case "THIS", "SELF", "SUPER":
		ident := &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
		p.nextToken()
		return ident
	}

	return &ast.NilLiteral{Token: p.curTok}
}

func (p *Parser) parseCallArguments() []ast.Expression {
	p.nextToken()
	args := []ast.Expression{}

	if p.curTok.Type == "RPAREN" {
		p.nextToken()
		return args
	}

	for {
		if p.curTok.Type == "ELLIPSIS" || p.peekTok.Type == "..." {
			p.nextToken()
		}
		args = append(args, p.parseExpression())
		if p.curTok.Type == "COMMA" {
			p.nextToken()
			continue
		}
		break
	}

	if p.curTok.Type == "RPAREN" {
		p.nextToken()
	}

	return args
}

func (p *Parser) parseArrayOrComprehension() ast.Expression {
	p.nextToken()
	elements := []ast.Expression{}

	if p.curTok.Type == "RBRACKET" {
		p.nextToken()
		return &ast.ArrayLiteral{Token: p.curTok, Elements: elements}
	}

	if p.curTok.Type == "FOR" {
		item := &ast.Identifier{Token: p.curTok, Value: "item"}
		if ident, ok := elements[0].(*ast.Identifier); ok {
			item = ident
		}
		p.nextToken()

		if p.curTok.Type == "IDENT" {
			item = &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
			p.nextToken()
		}

		if p.curTok.Type != "IN" {
			p.peekError("IN")
			return &ast.ArrayLiteral{Token: p.curTok, Elements: elements}
		}
		p.nextToken()

		iterable := p.parseExpression()

		var condition ast.Expression
		if p.curTok.Type == "IF" {
			p.nextToken()
			condition = p.parseExpression()
		}

		if p.curTok.Type == "RBRACKET" {
			p.nextToken()
		}

		return &ast.ListComprehension{Token: p.curTok, Element: item, Variable: item, Iterable: iterable, Condition: condition}
	}

	for {
		if p.curTok.Type == "RBRACKET" || p.curTok.Type == "EOF" || p.curTok.Type == "SEMICOLON" || p.curTok.Type == "NEWLINE" {
			if p.curTok.Type == "RBRACKET" {
				p.nextToken()
			}
			break
		}
		elements = append(elements, p.parseExpression())
		if p.curTok.Type == "COMMA" {
			p.nextToken()
		}
	}

	return &ast.ArrayLiteral{Token: p.curTok, Elements: elements}
}

func (p *Parser) parseSetLiteral() ast.Expression {
	p.nextToken()
	elements := []ast.Expression{}

	for p.curTok.Type != "RBRACE" && p.curTok.Type != "EOF" {
		elements = append(elements, p.parseExpression())
		if p.curTok.Type == "COMMA" {
			p.nextToken()
		}
	}

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.SetLiteral{Token: p.curTok, Elements: elements}
}

func (p *Parser) parseMapOrBlock() ast.Expression {
	if p.peekTok.Type == "IDENT" && (p.peekTok.Literal == "if" || p.peekTok.Literal == "for" || p.peekTok.Literal == "while") {
		return &ast.MapLiteral{Token: p.curTok, Pairs: []*ast.MapPair{}}
	}
	return p.parseMapLiteral()
}

func (p *Parser) parseMapLiteral() ast.Expression {
	p.nextToken()
	pairs := []*ast.MapPair{}

	for p.curTok.Type != "RBRACE" && p.curTok.Type != "EOF" {
		key := p.parseExpression()
		if p.curTok.Type == "COLON" || p.curTok.Type == "ASSIGN" || p.curTok.Type == "COLON_ASSIGN" {
			p.nextToken()
		}
		val := p.parseExpression()
		pairs = append(pairs, &ast.MapPair{Key: key, Value: val})
		if p.curTok.Type == "COMMA" {
			p.nextToken()
		}
	}

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.MapLiteral{Token: p.curTok, Pairs: pairs}
}

func (p *Parser) parseLambdaOrFunction() ast.Expression {
	tok := p.curTok
	isAsync := false

	if p.curTok.Type == "ASYNC" {
		isAsync = true
		p.nextToken()
	}

	if p.curTok.Type != "FUNCTION" && p.curTok.Type != "IDENT" {
		return &ast.NilLiteral{Token: p.curTok}
	}

	if p.curTok.Type == "FUNCTION" {
		p.nextToken()
	}

	var params []*ast.FunctionParameter
	if p.curTok.Type == "LPAREN" {
		params = p.parseFunctionParams()
	} else if p.curTok.Type == "IDENT" {
		params = []*ast.FunctionParameter{{Name: &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}}}
		p.nextToken()
	}

	if p.curTok.Type == "ARROW" || p.curTok.Type == "RETURN_ARROW" {
		p.nextToken()
		expr := p.parseExpression()
		return &ast.LambdaExpression{Token: tok, Parameters: params, Expression: expr, IsAsync: isAsync}
	}

	if p.peekTok.Type == "ARROW" {
		p.nextToken()
		expr := p.parseExpression()
		return &ast.LambdaExpression{Token: tok, Parameters: params, Expression: expr, IsAsync: isAsync}
	}

	body := p.parseBlock()

	if p.curTok.Type == "RBRACE" {
		p.nextToken()
	}

	return &ast.LambdaExpression{Token: tok, Parameters: params, Body: body, IsAsync: isAsync}
}

func (p *Parser) parseTypeExpression() *ast.TypeExpression {
	name := p.curTok.Literal
	p.nextToken()

	typ := &ast.TypeExpression{Token: p.curTok, Name: name}

	if p.curTok.Type == "LT" {
		p.nextToken()
		generics := []string{p.curTok.Literal}
		p.nextToken()
		for p.curTok.Type == "COMMA" {
			p.nextToken()
			generics = append(generics, p.curTok.Literal)
			p.nextToken()
		}
		if p.curTok.Type == "GT" {
			p.nextToken()
		}
		typ.Generics = generics
	}

	return typ
}

func (p *Parser) String() string {
	var out strings.Builder
	for _, imp := range p.prog.Imports {
		out.WriteString(imp.String())
		out.WriteByte('\n')
	}
	for _, stmt := range p.prog.Statements {
		out.WriteString(stmt.String())
		out.WriteByte('\n')
	}
	return out.String()
}
