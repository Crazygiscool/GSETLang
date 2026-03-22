package lexer

import (
	"gsetlang/ast"
	"strings"
	"unicode"
)

type Lexer struct {
	input        string
	position     int
	readPosition int
	ch           byte
	line         int
	column       int
}

func New(input string) *Lexer {
	l := &Lexer{
		input: input,
		line:  1,
	}
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
	l.column++
	if l.ch == '\n' {
		l.line++
		l.column = 0
	}
}

func (l *Lexer) peekChar() byte {
	if l.readPosition >= len(l.input) {
		return 0
	}
	return l.input[l.readPosition]
}

func (l *Lexer) peekChar2() byte {
	if l.readPosition+1 >= len(l.input) {
		return 0
	}
	return l.input[l.readPosition+1]
}

func (l *Lexer) NextToken() ast.Token {
	var tok ast.Token
	tok.Line = l.line
	tok.Column = l.column

	l.skipWhitespaceAndComments()

	if l.ch == 0 {
		return ast.Token{Type: "EOF", Literal: "", Line: l.line, Column: l.column}
	}

	if l.ch == '\n' || l.ch == '\r' {
		tok = ast.Token{Type: "NEWLINE", Literal: string(l.ch), Line: l.line, Column: l.column}
		l.readChar()
		return tok
	}

	switch l.ch {
	case '(':
		tok = ast.Token{Type: "LPAREN", Literal: "(", Line: l.line, Column: l.column}
	case ')':
		tok = ast.Token{Type: "RPAREN", Literal: ")", Line: l.line, Column: l.column}
	case '[':
		tok = ast.Token{Type: "LBRACKET", Literal: "[", Line: l.line, Column: l.column}
	case ']':
		tok = ast.Token{Type: "RBRACKET", Literal: "]", Line: l.line, Column: l.column}
	case '{':
		tok = ast.Token{Type: "LBRACE", Literal: "{", Line: l.line, Column: l.column}
	case '}':
		tok = ast.Token{Type: "RBRACE", Literal: "}", Line: l.line, Column: l.column}
	case ',':
		tok = ast.Token{Type: "COMMA", Literal: ",", Line: l.line, Column: l.column}
	case '.':
		if l.peekChar() == '.' && l.peekChar2() == '.' {
			l.readChar()
			l.readChar()
			tok = ast.Token{Type: "ELLIPSIS", Literal: "...", Line: l.line, Column: l.column}
		} else if l.peekChar() == '?' {
			l.readChar()
			tok = ast.Token{Type: "DOT_QUESTION", Literal: ".?", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "DOT", Literal: ".", Line: l.line, Column: l.column}
		}
	case ';':
		tok = ast.Token{Type: "SEMICOLON", Literal: ";", Line: l.line, Column: l.column}
	case ':':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "COLON_ASSIGN", Literal: ":=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "COLON", Literal: ":", Line: l.line, Column: l.column}
		}
	case '?':
		if l.peekChar() == '?' {
			l.readChar()
			tok = ast.Token{Type: "NULL_COALESCE", Literal: "??", Line: l.line, Column: l.column}
		} else if l.peekChar() == '.' {
			l.readChar()
			tok = ast.Token{Type: "QUESTION_DOT", Literal: "?.", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "QUESTION", Literal: "?", Line: l.line, Column: l.column}
		}
	case '@':
		tok = ast.Token{Type: "AT", Literal: "@", Line: l.line, Column: l.column}
	case '#':
		if l.peekChar() == '{' {
			tok = ast.Token{Type: "HASH_BRACE", Literal: "#{", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "HASH", Literal: "#", Line: l.line, Column: l.column}
		}
	case '`':
		tok.Type = "TEMPLATE_STRING"
		tok.Literal = l.readTemplateString()
		tok.Line = l.line
		tok.Column = l.column
		return tok
	case '"':
		if l.peekChar() == '"' && l.peekChar2() == '"' {
			tok.Type = "RAW_STRING"
			tok.Literal = l.readRawString()
		} else {
			tok.Type = "STRING"
			tok.Literal = l.readString()
			l.readChar()
		}
		tok.Line = l.line
		tok.Column = l.column
		return tok
	case '\'':
		tok = l.readCharLiteral()
		tok.Line = l.line
		tok.Column = l.column
		return tok
	case '=':
		if l.peekChar() == '=' {
			l.readChar()
			if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "STRICT_EQ", Literal: "===", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "EQ", Literal: "==", Line: l.line, Column: l.column}
			}
		} else if l.peekChar() == '>' {
			l.readChar()
			tok = ast.Token{Type: "ARROW", Literal: "=>", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "ASSIGN", Literal: "=", Line: l.line, Column: l.column}
		}
	case '+':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "PLUS_EQ", Literal: "+=", Line: l.line, Column: l.column}
		} else if l.peekChar() == '+' {
			l.readChar()
			tok = ast.Token{Type: "PLUS_PLUS", Literal: "++", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "PLUS", Literal: "+", Line: l.line, Column: l.column}
		}
	case '-':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "MINUS_EQ", Literal: "-=", Line: l.line, Column: l.column}
		} else if l.peekChar() == '-' {
			l.readChar()
			tok = ast.Token{Type: "MINUS_MINUS", Literal: "--", Line: l.line, Column: l.column}
		} else if l.peekChar() == '>' {
			l.readChar()
			tok = ast.Token{Type: "RETURN_ARROW", Literal: "->", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "MINUS", Literal: "-", Line: l.line, Column: l.column}
		}
	case '*':
		if l.peekChar() == '*' {
			l.readChar()
			if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "POWER_EQ", Literal: "**=", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "POWER", Literal: "**", Line: l.line, Column: l.column}
			}
		} else if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "MUL_EQ", Literal: "*=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "ASTERISK", Literal: "*", Line: l.line, Column: l.column}
		}
	case '/':
		if l.peekChar() == '/' {
			l.readComment()
			return l.NextToken()
		} else if l.peekChar() == '*' {
			l.readBlockComment()
			return l.NextToken()
		} else if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "DIV_EQ", Literal: "/=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "SLASH", Literal: "/", Line: l.line, Column: l.column}
		}
	case '%':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "MOD_EQ", Literal: "%=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "MOD", Literal: "%", Line: l.line, Column: l.column}
		}
	case '&':
		if l.peekChar() == '&' {
			l.readChar()
			if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "AND_EQ", Literal: "&&=", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "AND", Literal: "&&", Line: l.line, Column: l.column}
			}
		} else if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "BITAND_EQ", Literal: "&=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "BITAND", Literal: "&", Line: l.line, Column: l.column}
		}
	case '|':
		if l.peekChar() == '|' {
			l.readChar()
			if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "OR_EQ", Literal: "||=", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "OR", Literal: "||", Line: l.line, Column: l.column}
			}
		} else if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "BITOR_EQ", Literal: "|=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "BITOR", Literal: "|", Line: l.line, Column: l.column}
		}
	case '^':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "XOR_EQ", Literal: "^=", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "XOR", Literal: "^", Line: l.line, Column: l.column}
		}
	case '!':
		if l.peekChar() == '=' {
			l.readChar()
			if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "STRICT_NEQ", Literal: "!==", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "NEQ", Literal: "!=", Line: l.line, Column: l.column}
			}
		} else {
			tok = ast.Token{Type: "BANG", Literal: "!", Line: l.line, Column: l.column}
		}
	case '<':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "LTE", Literal: "<=", Line: l.line, Column: l.column}
		} else if l.peekChar() == '<' {
			l.readChar()
			if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "LSHIFT_EQ", Literal: "<<=", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "LSHIFT", Literal: "<<", Line: l.line, Column: l.column}
			}
		} else if l.peekChar() == '>' {
			l.readChar()
			tok = ast.Token{Type: "SPACESHIP", Literal: "<>", Line: l.line, Column: l.column}
		} else {
			tok = ast.Token{Type: "LT", Literal: "<", Line: l.line, Column: l.column}
		}
	case '>':
		if l.peekChar() == '=' {
			l.readChar()
			tok = ast.Token{Type: "GTE", Literal: ">=", Line: l.line, Column: l.column}
		} else if l.peekChar() == '>' {
			l.readChar()
			if l.peekChar() == '>' {
				l.readChar()
				if l.peekChar() == '=' {
					l.readChar()
					tok = ast.Token{Type: "RSHIFT_UNSIGNED_EQ", Literal: ">>>=", Line: l.line, Column: l.column}
				} else {
					tok = ast.Token{Type: "RSHIFT_UNSIGNED", Literal: ">>>", Line: l.line, Column: l.column}
				}
			} else if l.peekChar() == '=' {
				l.readChar()
				tok = ast.Token{Type: "RSHIFT_EQ", Literal: ">>=", Line: l.line, Column: l.column}
			} else {
				tok = ast.Token{Type: "RSHIFT", Literal: ">>", Line: l.line, Column: l.column}
			}
		} else {
			tok = ast.Token{Type: "GT", Literal: ">", Line: l.line, Column: l.column}
		}
	case 0:
		tok = ast.Token{Type: "EOF", Literal: "", Line: l.line, Column: l.column}
	default:
		if isLetter(l.ch) {
			tok.Literal = l.readIdentifier()
			tok.Type = lookupIdent(tok.Literal)
			tok.Line = l.line
			tok.Column = l.column
			return tok
		} else if isDigit(l.ch) {
			tok = l.readNumber()
			tok.Line = l.line
			tok.Column = l.column
			return tok
		}
		tok = ast.Token{Type: "ILLEGAL", Literal: string(l.ch), Line: l.line, Column: l.column}
	}

	l.readChar()
	return tok
}

func (l *Lexer) skipWhitespaceAndComments() {
	for l.ch == ' ' || l.ch == '\t' || l.ch == ',' {
		l.readChar()
	}
}

func (l *Lexer) readComment() {
	for l.ch != '\n' && l.ch != 0 {
		l.readChar()
	}
}

func (l *Lexer) readBlockComment() {
	l.readChar()
	l.readChar()
	for l.ch != 0 {
		if l.ch == '*' && l.peekChar() == '/' {
			l.readChar()
			l.readChar()
			return
		}
		l.readChar()
	}
}

func (l *Lexer) readString() string {
	pos := l.position + 1
	escape := false
	for {
		l.readChar()
		if l.ch == '\\' && !escape {
			escape = true
			continue
		}
		if l.ch == '"' && !escape {
			break
		}
		if l.ch == 0 {
			l.position = pos - 1
			break
		}
		escape = false
	}
	if pos >= len(l.input) {
		return ""
	}
	if l.position > len(l.input) {
		return l.input[pos:]
	}
	return l.input[pos:l.position]
}

func (l *Lexer) readRawString() string {
	l.readChar()
	l.readChar()
	l.readChar()
	pos := l.position
	for {
		if l.ch == '"' && l.peekChar() == '"' && l.peekChar2() == '"' {
			l.readChar()
			l.readChar()
			l.readChar()
			break
		}
		if l.ch == 0 {
			break
		}
		l.readChar()
	}
	return l.input[pos:l.position]
}

func (l *Lexer) readTemplateString() string {
	pos := l.position + 1
	for {
		l.readChar()
		if l.ch == '`' {
			break
		}
		if l.ch == '\\' {
			l.readChar()
			continue
		}
		if l.ch == 0 {
			break
		}
	}
	return l.input[pos:l.position]
}

func (l *Lexer) readCharLiteral() ast.Token {
	l.readChar()
	var ch string
	if l.ch == '\\' {
		l.readChar()
		ch = string(l.escapeChar(l.ch))
	} else {
		ch = string(l.ch)
	}
	l.readChar()
	if l.ch != '\'' {
		return ast.Token{Type: "ILLEGAL", Literal: "'"}
	}
	return ast.Token{Type: "CHAR", Literal: ch}
}

func (l *Lexer) escapeChar(ch byte) byte {
	switch ch {
	case 'n':
		return '\n'
	case 't':
		return '\t'
	case 'r':
		return '\r'
	case '\\':
		return '\\'
	case '\'':
		return '\''
	case '"':
		return '"'
	case '0':
		return 0
	default:
		return ch
	}
}

func (l *Lexer) readIdentifier() string {
	pos := l.position
	for isLetter(l.ch) || isDigit(l.ch) || l.ch == '!' || l.ch == '?' {
		l.readChar()
	}
	return l.input[pos:l.position]
}

func (l *Lexer) readNumber() ast.Token {
	pos := l.position
	isFloat := false

	if l.ch == '0' {
		if l.peekChar() == 'b' || l.peekChar() == 'B' {
			l.readChar()
			for l.ch == '0' || l.ch == '1' || l.ch == '_' {
				l.readChar()
			}
			lit := strings.Replace(l.input[pos:l.position], "_", "", -1)
			return ast.Token{Type: "INT", Literal: lit}
		} else if l.peekChar() == 'o' || l.peekChar() == 'O' {
			l.readChar()
			for (l.ch >= '0' && l.ch <= '7') || l.ch == '_' {
				l.readChar()
			}
			lit := strings.Replace(l.input[pos:l.position], "_", "", -1)
			return ast.Token{Type: "INT", Literal: lit}
		} else if l.peekChar() == 'x' || l.peekChar() == 'X' {
			l.readChar()
			for (l.ch >= '0' && l.ch <= '9') || (l.ch >= 'a' && l.ch <= 'f') || (l.ch >= 'A' && l.ch <= 'F') || l.ch == '_' {
				l.readChar()
			}
			lit := strings.Replace(l.input[pos:l.position], "_", "", -1)
			return ast.Token{Type: "INT", Literal: lit}
		}
	}

	for isDigit(l.ch) || l.ch == '_' {
		l.readChar()
	}

	if l.ch == '.' && isDigit(l.peekChar()) {
		isFloat = true
		l.readChar()
		for isDigit(l.ch) || l.ch == '_' {
			l.readChar()
		}
	}

	if l.ch == 'e' || l.ch == 'E' {
		isFloat = true
		l.readChar()
		if l.ch == '+' || l.ch == '-' {
			l.readChar()
		}
		for isDigit(l.ch) || l.ch == '_' {
			l.readChar()
		}
	}

	if l.ch == 'i' || l.ch == 'I' {
		l.readChar()
		return ast.Token{Type: "IMAG", Literal: l.input[pos:l.position]}
	}

	lit := l.input[pos:l.position]
	if isFloat {
		lit = strings.Replace(lit, "_", "", -1)
		return ast.Token{Type: "FLOAT", Literal: lit}
	}
	lit = strings.Replace(lit, "_", "", -1)
	return ast.Token{Type: "INT", Literal: lit}
}

func isLetter(ch byte) bool {
	return unicode.IsLetter(rune(ch)) || ch == '_'
}

func isDigit(ch byte) bool {
	return ch >= '0' && ch <= '9'
}

var keywords = map[string]string{
	// Declaration keywords
	"fn":       "FUNCTION",
	"func":     "FUNCTION",
	"def":      "FUNCTION",
	"function": "FUNCTION",
	"var":      "VAR",
	"val":      "VAL",
	"let":      "LET",
	"const":    "CONST",
	"mut":      "MUT",
	"static":   "STATIC",
	"final":    "FINAL",

	// Type keywords
	"type":       "TYPE",
	"class":      "CLASS",
	"struct":     "STRUCT",
	"enum":       "ENUM",
	"trait":      "TRAIT",
	"interface":  "INTERFACE",
	"impl":       "IMPL",
	"extends":    "EXTENDS",
	"implements": "IMPLEMENTS",
	"mixin":      "MIXIN",
	"public":     "PUBLIC",
	"private":    "PRIVATE",
	"protected":  "PROTECTED",
	"internal":   "INTERNAL",
	"readonly":   "READONLY",

	// Control flow
	"if":      "IF",
	"elif":    "ELIF",
	"else":    "ELSE",
	"match":   "MATCH",
	"case":    "CASE",
	"default": "DEFAULT",
	"switch":  "SWITCH",

	// Loops
	"for":      "FOR",
	"foreach":  "FOREACH",
	"in":       "IN",
	"while":    "WHILE",
	"do":       "DO",
	"break":    "BREAK",
	"continue": "CONTINUE",

	// Exceptions
	"try":     "TRY",
	"catch":   "CATCH",
	"throw":   "THROW",
	"throws":  "THROWS",
	"finally": "FINALLY",

	// Async
	"async":     "ASYNC",
	"await":     "AWAIT",
	"yield":     "YIELD",
	"generator": "GENERATOR",

	// Return/Exit
	"return": "RETURN",
	"exit":   "EXIT",
	"panic":  "PANIC",

	// Null
	"nil":       "NIL",
	"null":      "NULL",
	"none":      "NONE",
	"undefined": "UNDEFINED",

	// Booleans
	"true":  "TRUE",
	"false": "FALSE",
	"and":   "AND",
	"or":    "OR",
	"not":   "NOT",

	// Import/Export
	"import":  "IMPORT",
	"from":    "FROM",
	"export":  "EXPORT",
	"module":  "MODULE",
	"as":      "AS",
	"using":   "USING",
	"require": "REQUIRE",

	// OOP
	"new":         "NEW",
	"this":        "THIS",
	"self":        "SELF",
	"super":       "SUPER",
	"init":        "INIT",
	"constructor": "CONSTRUCTOR",
	"get":         "GET",
	"set":         "SET",
	"abstract":    "ABSTRACT",
	"override":    "OVERRIDE",
	"virtual":     "VIRTUAL",
	"sealed":      "SEALED",
	"open":        "OPEN",
	"lazy":        "LAZY",

	// Decorators
	"decorator": "DECORATOR",
	"@":         "AT",

	// Other
	"defer":    "DEFER",
	"goto":     "GOTO",
	"assert":   "ASSERT",
	"where":    "WHERE",
	"guard":    "GUARD",
	"is":       "IS",
	"as_type":  "AS_TYPE",
	"typeof":   "TYPEOF",
	"sizeof":   "SIZEOF",
	"inline":   "INLINE",
	"noinline": "NOINLINE",
}

var keywordAliases = map[string]string{
	"func": "fn",
	"def":  "fn",
	"val":  "let",
	"elif": "else if",
	"None": "nil",
	"self": "this",
	"and":  "&&",
	"or":   "||",
	"not":  "!",
}

func lookupIdent(s string) string {
	if tok, ok := keywords[s]; ok {
		return tok
	}
	return "IDENT"
}

// Position returns the current position in the input
func (l *Lexer) Position() (int, int) {
	return l.line, l.column
}
