package main

func main() {
    // A sample input string with a header and code
    testInput := "shout=PRINT\nstop=EXIT\n---\nshout \"Hello from iSH!\""
    
    config, body := ParseGSet(testInput)
    
    import "fmt" // Make sure to add "fmt" to your imports at the top!
    fmt.Println("Keywords found:", config.Keywords)
    fmt.Println("Code body:", body)
}