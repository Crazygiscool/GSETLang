package lexer

import "gsetlang/ast"

type Lexer struct {
	input        string
	position     int
	readPosition int
	ch           byte
}

func New(input string) *Lexer {
	l := &Lexer{input: input}
	l.readChar()
	return l
}

func (l *Lexer) readChar() {
	if l.readPosition >= len(l.input) {
		l.ch = 0
	} else {
		l.ch = l.input[l.readPosition]
	}
	l.position = l.readPosition
	l.readPosition++
}

func (l *Lexer) peekChar() byte {
	if l.readPosition >= len(l.input) {
		return 0
	}
	return l.input[l.readPosition]
}

func (l *Lexer) NextToken() ast.Token {
	var tok ast.Token

	l.skipWhitespace()

	switch l.ch {
	case '(':
		tok = ast.Token{Type: "LPAREN", Literal: "("}
	case ')':
		tok = ast.Token{Type: "RPAREN", Literal: ")"}
	case '[':
		tok = ast.Token{Type: "LBRACKET", Literal: "["}
	case ']':
		tok = ast.Token{Type: "RBRACKET", Literal: "]"}
	case '{':
		tok = ast.Token{Type: "LBRACE", Literal: "{"}
	case '}':
		tok = ast.Token{Type: "RBRACE", Literal: "}"}
	case ',':
		tok = ast.Token{Type: "COMMA", Literal: ","}
	case '.':
		tok = ast.Token{Type: "DOT", Literal: "."}
	case ';':
		tok = ast.Token{Type: "SEMICOLON", Literal: ";"}
	case ':':
		tok = ast.Token{Type: "COLON", Literal: ":"}
	case '"':
		tok.Type = "STRING"
		tok.Literal = l.readString()
	case '=':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "EQ", Literal: "=="}
		} else {
			tok = ast.Token{Type: "ASSIGN", Literal: "="}
		}
	case '+':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "PLUS_EQ", Literal: "+="}
		} else {
			tok = ast.Token{Type: "PLUS", Literal: "+"}
		}
	case '-':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "MINUS_EQ", Literal: "-="}
		} else {
			tok = ast.Token{Type: "MINUS", Literal: "-"}
		}
	case '*':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "MUL_EQ", Literal: "*="}
		} else {
			tok = ast.Token{Type: "ASTERISK", Literal: "*"}
		}
	case '/':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "DIV_EQ", Literal: "/="}
		} else if l.peekChar() == '/' {
			l.readComment()
			return l.NextToken()
		} else {
			tok = ast.Token{Type: "SLASH", Literal: "/"}
		}
	case '%':
		tok = ast.Token{Type: "MOD", Literal: "%"}
	case '!':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "NEQ", Literal: "!="}
		} else {
			tok = ast.Token{Type: "BANG", Literal: "!"}
		}
	case '<':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "LTE", Literal: "<="}
		} else if l.peekChar() == '<' {
			l.readChar()
			tok = ast.Token{Type: "LSHIFT", Literal: "<<"}
		} else {
			tok = ast.Token{Type: "LT", Literal: "<"}
		}
	case '>':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "GTE", Literal: ">="}
		} else if l.peekChar() == '>' {
			l.readChar()
			tok = ast.Token{Type: "RSHIFT", Literal: ">>"}
		} else {
			tok = ast.Token{Type: "GT", Literal: ">"}
		}
	case '&':
		if l.peekChar() == '&' {
			l.readChar()
			tok = ast.Token{Type: "AND", Literal: "&&"}
		} else {
			tok = ast.Token{Type: "BITAND", Literal: "&"}
		}
	case '|':
		if l.peekChar() == '|' {
			l.readChar()
			tok = ast.Token{Type: "OR", Literal: "||"}
		} else {
			tok = ast.Token{Type: "BITOR", Literal: "|"}
		}
	case 0:
		tok = ast.Token{Type: "EOF", Literal: ""}
	default:
		if isLetter(l.ch) {
			tok.Literal = l.readIdentifier()
			tok.Type = lookupIdent(tok.Literal)
			return tok
		} else if isDigit(l.ch) {
			tok = l.readNumber()
			return tok
		}
		tok = ast.Token{Type: "ILLEGAL", Literal: string(l.ch)}
	}

	l.readChar()
	return tok
}

func (l *Lexer) skipWhitespace() {
	for l.ch == ' ' || l.ch == '\t' || l.ch == '\n' || l.ch == '\r' {
		l.readChar()
	}
}

func (l *Lexer) readComment() {
	for l.ch != '\n' && l.ch != 0 {
		l.readChar()
	}
	l.skipWhitespace()
}

func (l *Lexer) readString() string {
	pos := l.position + 1
	for {
		l.readChar()
		if l.ch == '"' || l.ch == 0 {
			break
		}
	}
	return l.input[pos:l.position]
}

func (l *Lexer) readIdentifier() string {
	pos := l.position
	for isLetter(l.ch) || isDigit(l.ch) {
		l.readChar()
	}
	return l.input[pos:l.position]
}

func (l *Lexer) readNumber() ast.Token {
	pos := l.position
	isFloat := false
	for isDigit(l.ch) {
		l.readChar()
	}
	if l.ch == '.' && isDigit(l.peekChar()) {
		isFloat = true
		l.readChar()
		for isDigit(l.ch) {
			l.readChar()
		}
	}
	lit := l.input[pos:l.position]
	if isFloat {
		return ast.Token{Type: "FLOAT", Literal: lit}
	}
	return ast.Token{Type: "INT", Literal: lit}
}

func isLetter(ch byte) bool {
	return 'a' <= ch && ch <= 'z' || 'A' <= ch && ch <= 'Z' || ch == '_'
}

func isDigit(ch byte) bool {
	return '0' <= ch && ch <= '9'
}

var keywords = map[string]string{
	"fn":       "FUNCTION",
	"var":      "VAR",
	"let":      "LET",
	"const":    "CONST",
	"if":       "IF",
	"else":     "ELSE",
	"elif":     "ELIF",
	"for":      "FOR",
	"while":    "WHILE",
	"switch":   "SWITCH",
	"case":     "CASE",
	"break":    "BREAK",
	"continue": "CONTINUE",
	"return":   "RETURN",
	"true":     "TRUE",
	"false":    "FALSE",
	"nil":      "NIL",
	"null":     "NULL",
	"class":    "CLASS",
	"struct":   "STRUCT",
	"trait":    "TRAIT",
	"impl":     "IMPL",
	"pub":      "PUBLIC",
	"priv":     "PRIVATE",
	"import":   "IMPORT",
	"module":   "MODULE",
	"new":      "NEW",
	"this":     "THIS",
	"super":    "SUPER",
}

func lookupIdent(s string) string {
	if tok, ok := keywords[s]; ok {
		return tok
	}
	return "IDENT"
}
