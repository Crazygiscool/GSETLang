package main

import "fmt"

type Parser struct {
	l         *Lexer
	curToken  Token
	peekToken Token
	errors    []string
	program   []Statement
}

func NewParser(l *Lexer) *Parser {
	p := &Parser{l: l, errors: []string{}}
	p.nextToken()
	p.nextToken()
	p.program = p.ParseProgram()
	return p
}

func (p *Parser) nextToken() {
	p.curToken = p.peekToken
	p.peekToken = p.l.NextToken()
}

func (p *Parser) isPeek(t TokenType) bool {
	return p.peekToken.Type == t
}

func (p *Parser) expectPeek(t TokenType) bool {
	if p.isPeek(t) {
		p.nextToken()
		return true
	}
	p.peekError(t)
	return false
}

func (p *Parser) peekError(t TokenType) {
	msg := fmt.Sprintf("expected next token to be %s, got %s instead", t, p.peekToken.Type)
	p.errors = append(p.errors, msg)
}

func (p *Parser) Errors() []string {
	return p.errors
}

func (p *Parser) parseCallExpression() *CallExpression {
	ce := &CallExpression{
		Token:    p.curToken,
		Function: p.curToken.Literal,
	}

	if !p.expectPeek(TOK_LPAREN) {
		return nil
	}

	ce.Arguments = p.parseExpressionList()

	if !p.expectPeek(TOK_RPAREN) {
		return nil
	}

	return ce
}

func (p *Parser) parseExpressionList() []Expression {
	list := []Expression{}

	if p.isPeek(TOK_RPAREN) {
		return list
	}

	p.nextToken()

	expr := &StringLiteral{Token: p.curToken, Value: p.curToken.Literal}
	list = append(list, expr)

	for p.isPeek(TOK_STRING) {
		p.nextToken()
		expr := &StringLiteral{Token: p.curToken, Value: p.curToken.Literal}
		list = append(list, expr)
	}

	return list
}

func (p *Parser) ParseProgram() []Statement {
	stmts := []Statement{}

	for p.curToken.Type != TOK_EOF {
		stmt := p.parseStatement()
		if stmt != nil {
			stmts = append(stmts, stmt)
		}
		p.nextToken()
	}

	return stmts
}

func (p *Parser) parseStatement() Statement {
	if p.curToken.Type == TOK_IDENT && p.isPeek(TOK_LPAREN) {
		return p.parseCallExpression()
	}
	return nil
}

func (p *Parser) String() string {
	var out string
	for _, stmt := range p.program {
		out += stmt.String() + "\n"
	}
	return out
}
