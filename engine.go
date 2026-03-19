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

func Translate(conf GSETConfig, body string) string {

	body = strings.ReplaceAll(body, "(", " ( ")
	body = strings.ReplaceAll(body, ")", " ) ")
	body = strings.ReplaceAll(body, "\"", " \" ")

	words := strings.Fields(body)

	var translated []string

	for _, word := range words {
		newWord, exists := conf.Keywords[word]

		if exists {
			translated = append(translated, newWord)
		} else {
			translated = append(translated, word)
		}
	}

	return strings.Join(translated, "")
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
