package main

import (
	"fmt"
	"os"
)

func main() {
	// A sample input string with a header and code
	testInput := fileparse("./test/test.gset")

	config, body := ParseGSet(testInput)
	translated := Translate(config, body)
	fmt.Println("Keywords found:", config.Keywords)
	fmt.Println("Code body:", body)
	fmt.Println("Translated: ", translated)

	Execute(translated)

}

func fileparse(filepath string) string {
	content, err := os.ReadFile(filepath)

	if err != nil {
		fmt.Println("Error reading file:", err)
		return ""
	}

	return string(content)
}
