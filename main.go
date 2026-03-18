package main

import "fmt"

func main() {
    // A sample input string with a header and code
    testInput := "shout=PRINT\nstop=EXIT\n---\nshout \"Hello from Crazygiscool!\""

    config, body := ParseGSet(testInput)
    fmt.Println("Keywords found:", config.Keywords)
    fmt.Println("Code body:", body)
}
