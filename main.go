package main

import (
	"fmt"
	"os"
	"strings"
)

func main() {
	testInput := fileparse("./test/test.gset")
	config, body := ParseGSet(testInput)

	l := NewLexer(body)
	var translatedParts []string

	for {
		tok := l.NextToken()
		if tok.Type == TOK_EOF {
			break
		}

		// Check if IDENT is a custom keyword from config
		val, exists := config.Keywords[tok.Literal]

		if tok.Type == TOK_IDENT && exists {
			translatedParts = append(translatedParts, val)
		} else if tok.Type == TOK_STRING {
			translatedParts = append(translatedParts, fmt.Sprintf(`"%s"`, tok.Literal))
		} else {
			translatedParts = append(translatedParts, tok.Literal)
		}
	}

	finalCode := strings.Join(translatedParts, "")
	fmt.Println("Final Code to Exec:", finalCode)
	Execute(finalCode)
}

func fileparse(filepath string) string {
	content, err := os.ReadFile(filepath)

	if err != nil {
		fmt.Println("Error reading file:", err)
		return ""
	}

	return string(content)
}
