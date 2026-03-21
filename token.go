package main

type TokenType string

const (
	TOK_IDENT   = "IDENT"   // shout, name, x
	TOK_STRING  = "STRING"  // "Hello"
	TOK_LPAREN  = "LPAREN"  // (
	TOK_RPAREN  = "RPAREN"  // )
	TOK_EOF     = "EOF"     // End of file
	TOK_ILLEGAL = "ILLEGAL" // Unknown character
)

type Token struct {
	Type    TokenType
	Literal string
}
