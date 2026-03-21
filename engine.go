package main

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
)

type GSETConfig struct {
	Keywords map[string]string //Keywords var with the type of map and input string, output string. pair
}

func ParseGSet(src string) (GSETConfig, string) { //output only defined the type of outputs

	conf := GSETConfig{Keywords: make(map[string]string)}

	parts := strings.SplitN(src, "---", 2)

	//if code part after split is lesser than 2
	if len(parts) < 2 {
		//return config, which is blank, and the spurce code
		return conf, src

	}

	header := parts[0]                   // the config header
	lines := strings.Split(header, "\n") //take every line out from header with return key

	for _, line := range lines { //for every created var line in lines
		pair := strings.SplitN(line, "=", 2) //split the part before equals and after equals

		if len(pair) == 2 {
			//def key and val trimpped of spaces
			key := strings.TrimSpace(pair[0])
			val := strings.TrimSpace(pair[1])

			//map to conf
			conf.Keywords[key] = val

		}
	}

	return conf, parts[1] //not empty, so we return config and second part, the code body

}

type Lexer struct {
	input        string
	position     int  // current position (current char)
	readPosition int  // next position (after current char)
	ch           byte // current char under examination
}

func NewLexer(input string) *Lexer {
	l := &Lexer{input: input}
	l.readChar()
	return l
}

func (l *Lexer) readChar() {
	if l.readPosition >= len(l.input) {
		l.ch = 0 // End of input
	} else {
		l.ch = l.input[l.readPosition]
	}
	l.position = l.readPosition
	l.readPosition++
}

func (l *Lexer) NextToken() Token {
	var tok Token
	l.skipWhitespace()

	switch l.ch {
	case '(':
		tok = Token{TOK_LPAREN, "("}
	case ')':
		tok = Token{TOK_RPAREN, ")"}
	case '"':
		tok.Type = TOK_STRING
		tok.Literal = l.readString()
	case 0:
		tok.Type = TOK_EOF
		tok.Literal = ""
	default:
		if isLetter(l.ch) {
			tok.Literal = l.readIdentifier()
			tok.Type = TOK_IDENT
			return tok
		}
		tok = Token{TOK_ILLEGAL, string(l.ch)}
	}

	l.readChar()
	return tok
}

// Helpers
func (l *Lexer) skipWhitespace() {
	for l.ch == ' ' || l.ch == '\t' || l.ch == '\n' || l.ch == '\r' {
		l.readChar()
	}
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
	for isLetter(l.ch) {
		l.readChar()
	}
	return l.input[pos:l.position]
}

func isLetter(ch byte) bool {
	return 'a' <= ch && ch <= 'z' || 'A' <= ch && ch <= 'Z' || ch == '_'
}

func Execute(translatedCode string) {
	wrapper := fmt.Sprintf(`package main
import "fmt"
func main() {
    %s
}`, translatedCode)

	err := os.WriteFile("temp_exec.go", []byte(wrapper), 0644)
	if err != nil {
		fmt.Println("Error creating temp file:", err)
		return
	}

	cmd := exec.Command("go", "run", "temp_exec.go")

	// Connect the command's output to terminal
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	fmt.Println("--- RUNNING GSET OUTPUT ---")
	cmd.Run()

	// Clean up (Optional: delete the temp file)
	//os.Remove("temp_exec.go")
}
