package parser

import (
	"fmt"
	"gsetlang/ast"
	"gsetlang/lexer"
	"strconv"
)

type Parser struct {
	l       *lexer.Lexer
	curTok  ast.Token
	peekTok ast.Token
	errors  []string
}

func New(l *lexer.Lexer) *Parser {
	p := &Parser{l: l, errors: []string{}}
	p.nextToken()
	p.nextToken()
	return p
}

func (p *Parser) nextToken() {
	p.curTok = p.peekTok
	p.peekTok = p.l.NextToken()
}

func (p *Parser) Errors() []string {
	return p.errors
}

func (p *Parser) peekError(t string) {
	msg := fmt.Sprintf("expected next token to be %s, got %s", t, p.peekTok.Type)
	p.errors = append(p.errors, msg)
}

func (p *Parser) ParseProgram() *ast.Program {
	prog := &ast.Program{Statements: []ast.Statement{}}

	for p.curTok.Type != "EOF" {
		stmt := p.parseStatement()
		if stmt != nil {
			prog.Statements = append(prog.Statements, stmt)
		}
		p.nextToken()
	}

	return prog
}

func (p *Parser) parseStatement() ast.Statement {
	switch p.curTok.Type {
	case "VAR", "LET":
		return p.parseVariableStatement()
	case "IF":
		return p.parseIfStatement()
	case "FOR":
		return p.parseForStatement()
	case "WHILE":
		return p.parseWhileStatement()
	case "FN":
		return p.parseFunctionStatement()
	case "RETURN":
		return p.parseReturnStatement()
	case "BREAK":
		return p.parseBreakStatement()
	case "CONTINUE":
		return p.parseContinueStatement()
	default:
		return p.parseExpressionStatement()
	}
}

func (p *Parser) parseVariableStatement() ast.Statement {
	name := p.curTok.Literal
	p.nextToken()

	if p.curTok.Type != "ASSIGN" {
		p.peekError("ASSIGN")
		return nil
	}
	p.nextToken()

	val := p.parseExpression()

	return &ast.VariableStatement{
		Token: p.curTok,
		Name:  name,
		Value: val,
	}
}

func (p *Parser) parseAssignmentStatement(name string) ast.Statement {
	op := p.curTok.Literal
	p.nextToken()

	val := p.parseExpression()

	return &ast.AssignmentStatement{
		Token:    p.curTok,
		Name:     name,
		Operator: op,
		Value:    val,
	}
}

func (p *Parser) parseIfStatement() ast.Statement {
	p.nextToken()
	condition := p.parseExpression()

	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return nil
	}
	consequence := p.parseBlockStatement()

	var alternative *ast.BlockStatement
	if p.curTok.Type == "ELSE" {
		p.nextToken()
		if p.curTok.Type == "IF" {
			alt := p.parseIfStatement()
			if block, ok := alt.(*ast.IfStatement); ok {
				alternative = &ast.BlockStatement{
					Token:      p.curTok,
					Statements: []ast.Statement{block},
				}
			}
		} else if p.curTok.Type == "LBRACE" {
			alternative = p.parseBlockStatement()
		}
	}

	return &ast.IfStatement{
		Token:       p.curTok,
		Condition:   condition,
		Consequence: consequence,
		Alternative: alternative,
	}
}

func (p *Parser) parseForStatement() ast.Statement {
	p.nextToken()

	var init ast.Statement
	var condition ast.Expression
	var update ast.Statement

	if p.curTok.Type != "SEMICOLON" {
		if p.curTok.Type == "VAR" || p.curTok.Type == "LET" {
			init = p.parseVariableStatement()
		}
	}

	if p.curTok.Type != "SEMICOLON" {
		condition = p.parseExpression()
	}

	if p.curTok.Type != "SEMICOLON" {
		p.nextToken()
	}

	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return nil
	}
	body := p.parseBlockStatement()

	return &ast.ForStatement{
		Token:     p.curTok,
		Init:      init.(*ast.VariableStatement),
		Condition: condition,
		Update:    update.(*ast.AssignmentStatement),
		Body:      body,
	}
}

func (p *Parser) parseWhileStatement() ast.Statement {
	p.nextToken()
	condition := p.parseExpression()

	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return nil
	}
	body := p.parseBlockStatement()

	return &ast.WhileStatement{
		Token:     p.curTok,
		Condition: condition,
		Body:      body,
	}
}

func (p *Parser) parseFunctionStatement() ast.Statement {
	p.nextToken()
	name := p.curTok.Literal
	p.nextToken()

	if p.curTok.Type != "LPAREN" {
		p.peekError("LPAREN")
		return nil
	}

	params := p.parseFunctionParams()

	if p.curTok.Type != "LBRACE" {
		p.peekError("LBRACE")
		return nil
	}
	body := p.parseBlockStatement()

	return &ast.FunctionStatement{
		Token:      p.curTok,
		Name:       name,
		Parameters: params,
		Body:       body,
	}
}

func (p *Parser) parseFunctionParams() []string {
	params := []string{}

	p.nextToken()
	if p.curTok.Type == "RPAREN" {
		p.nextToken()
		return params
	}

	params = append(params, p.curTok.Literal)
	p.nextToken()

	for p.curTok.Type == "COMMA" {
		p.nextToken()
		params = append(params, p.curTok.Literal)
		p.nextToken()
	}

	if p.curTok.Type != "RPAREN" {
		p.peekError("RPAREN")
		return nil
	}
	p.nextToken()

	return params
}

func (p *Parser) parseReturnStatement() ast.Statement {
	p.nextToken()

	var val ast.Expression
	if p.curTok.Type != "SEMICOLON" && p.curTok.Type != "EOF" {
		val = p.parseExpression()
	}

	return &ast.ReturnStatement{
		Token: p.curTok,
		Value: val,
	}
}

func (p *Parser) parseBreakStatement() ast.Statement {
	p.nextToken()
	return &ast.BreakStatement{Token: p.curTok}
}

func (p *Parser) parseContinueStatement() ast.Statement {
	p.nextToken()
	return &ast.ContinueStatement{Token: p.curTok}
}

func (p *Parser) parseBlockStatement() *ast.BlockStatement {
	p.nextToken()
	block := &ast.BlockStatement{
		Token:      p.curTok,
		Statements: []ast.Statement{},
	}

	for p.curTok.Type != "RBRACE" && p.curTok.Type != "EOF" {
		stmt := p.parseStatement()
		if stmt != nil {
			block.Statements = append(block.Statements, stmt)
		}
		p.nextToken()
	}

	return block
}

func (p *Parser) parseExpressionStatement() ast.Statement {
	expr := p.parseExpression()

	if p.curTok.Type == "ASSIGN" || p.curTok.Type == "PLUS_EQ" ||
		p.curTok.Type == "MINUS_EQ" || p.curTok.Type == "MUL_EQ" || p.curTok.Type == "DIV_EQ" {
		if ident, ok := expr.(*ast.Identifier); ok {
			return p.parseAssignmentStatement(ident.Value)
		}
	}

	return &ast.ExpressionStatement{
		Token:      p.curTok,
		Expression: expr,
	}
}

func (p *Parser) parseExpression() ast.Expression {
	return p.parseInfixExpression()
}

func (p *Parser) parseInfixExpression() ast.Expression {
	left := p.parsePrefixExpression()

	for p.peekTok.Type == "PLUS" || p.peekTok.Type == "MINUS" ||
		p.peekTok.Type == "ASTERISK" || p.peekTok.Type == "SLASH" ||
		p.peekTok.Type == "MOD" || p.peekTok.Type == "EQ" ||
		p.peekTok.Type == "NEQ" || p.peekTok.Type == "LT" ||
		p.peekTok.Type == "GT" || p.peekTok.Type == "LTE" ||
		p.peekTok.Type == "GTE" || p.peekTok.Type == "AND" ||
		p.peekTok.Type == "OR" {

		p.nextToken()
		op := p.curTok.Literal
		p.nextToken()
		right := p.parsePrefixExpression()

		left = &ast.InfixExpression{
			Token:    p.curTok,
			Left:     left,
			Operator: op,
			Right:    right,
		}
	}

	return left
}

func (p *Parser) parsePrefixExpression() ast.Expression {
	if p.curTok.Type == "BANG" || p.curTok.Type == "MINUS" {
		p.nextToken()
		right := p.parsePrefixExpression()
		return &ast.PrefixExpression{
			Token:    p.curTok,
			Operator: p.curTok.Literal,
			Right:    right,
		}
	}

	return p.parsePostfixExpression()
}

func (p *Parser) parsePostfixExpression() ast.Expression {
	left := p.parsePrimary()
	if left == nil {
		return nil
	}

	for p.peekTok.Type == "LPAREN" || p.peekTok.Type == "DOT" || p.peekTok.Type == "LBRACKET" {
		if p.peekTok.Type == "LPAREN" {
			p.nextToken()
			args := p.parseExpressionList()
			if ident, ok := left.(*ast.Identifier); ok {
				left = &ast.CallExpression{
					Token:     p.curTok,
					Function:  ident.Value,
					Arguments: args,
				}
			}
		} else if p.peekTok.Type == "DOT" {
			p.nextToken()
			p.nextToken()
			method := p.curTok.Literal
			if p.peekTok.Type == "LPAREN" {
				p.nextToken()
				args := p.parseExpressionList()
				if ident, ok := left.(*ast.Identifier); ok {
					left = &ast.CallExpression{
						Token:     p.curTok,
						Function:  ident.Value + "." + method,
						Arguments: args,
					}
				}
			}
		} else if p.peekTok.Type == "LBRACKET" {
			p.nextToken()
			index := p.parseExpression()
			if p.curTok.Type != "RBRACKET" {
				p.peekError("RBRACKET")
				return nil
			}
			p.nextToken()
			left = &ast.IndexExpression{
				Token: p.curTok,
				Left:  left,
				Index: index,
			}
		}
	}

	return left
}

func (p *Parser) parsePrimary() ast.Expression {
	switch p.curTok.Type {
	case "IDENT":
		return &ast.Identifier{Token: p.curTok, Value: p.curTok.Literal}
	case "INT":
		val, _ := strconv.ParseInt(p.curTok.Literal, 10, 64)
		return &ast.IntegerLiteral{Token: p.curTok, Value: val}
	case "FLOAT":
		val, _ := strconv.ParseFloat(p.curTok.Literal, 64)
		return &ast.FloatLiteral{Token: p.curTok, Value: val}
	case "STRING":
		return &ast.StringLiteral{Token: p.curTok, Value: p.curTok.Literal}
	case "TRUE":
		p.nextToken()
		return &ast.BooleanLiteral{Token: p.curTok, Value: true}
	case "FALSE":
		p.nextToken()
		return &ast.BooleanLiteral{Token: p.curTok, Value: false}
	case "LBRACKET":
		return p.parseArrayLiteral()
	case "LBRACE":
		return p.parseMapLiteral()
	case "LPAREN":
		p.nextToken()
		expr := p.parseExpression()
		if p.curTok.Type != "RPAREN" {
			p.peekError("RPAREN")
			return nil
		}
		p.nextToken()
		return expr
	}

	p.errors = append(p.errors, "unexpected token: "+p.curTok.Type)
	return nil
}

func (p *Parser) parseArrayLiteral() ast.Expression {
	p.nextToken()
	elts := []ast.Expression{}

	if p.curTok.Type != "RBRACKET" {
		elts = append(elts, p.parseExpression())
		for p.curTok.Type == "COMMA" {
			p.nextToken()
			elts = append(elts, p.parseExpression())
		}
	}

	if p.curTok.Type != "RBRACKET" {
		p.peekError("RBRACKET")
		return nil
	}
	p.nextToken()

	return &ast.ArrayLiteral{Token: p.curTok, Elements: elts}
}

func (p *Parser) parseMapLiteral() ast.Expression {
	p.nextToken()
	pairs := make(map[ast.Expression]ast.Expression)

	if p.curTok.Type != "RBRACE" {
		key := p.parseExpression()
		if p.curTok.Type != "COLON" {
			p.peekError("COLON")
			return nil
		}
		p.nextToken()
		val := p.parseExpression()
		pairs[key] = val

		for p.curTok.Type == "COMMA" {
			p.nextToken()
			key = p.parseExpression()
			if p.curTok.Type != "COLON" {
				p.peekError("COLON")
				return nil
			}
			p.nextToken()
			val = p.parseExpression()
			pairs[key] = val
		}
	}

	if p.curTok.Type != "RBRACE" {
		p.peekError("RBRACE")
		return nil
	}
	p.nextToken()

	return &ast.MapLiteral{Token: p.curTok, Pairs: pairs}
}

func (p *Parser) parseExpressionList() []ast.Expression {
	list := []ast.Expression{}

	p.nextToken()
	if p.curTok.Type == "RPAREN" || p.curTok.Type == "RBRACKET" {
		return list
	}

	list = append(list, p.parseExpression())

	for p.curTok.Type == "COMMA" {
		p.nextToken()
		list = append(list, p.parseExpression())
	}

	return list
}

func (p *Parser) String() string {
	var out string
	for _, stmt := range p.ParseProgram().Statements {
		out += stmt.String() + "\n"
	}
	return out
}
