To build a language exactly like Go—a compiled, statically typed language with its own runtime—you are moving away from "scripting" and into **Systems Programming**. 

Google’s Go compiler is a "Self-Hosting" compiler. It follows a very rigid, scientific pipeline. To build a customizable version of this, you need to master these 5 levels.

---

### The GSET Compiler Roadmap

#### Phase 1: The Lexer (Lexical Analysis)
* **Goal:** Turn raw text into a stream of **Tokens**.
* **The Go Way:** Instead of `strings.Split`, you write a "Scanner" that reads one character at a time. It identifies `func`, `(`, `"string"`, and `123`.
* **Customization:** This is where you define your **Keywords**. If you want `shout` to be a keyword, the Lexer must recognize those specific characters as a `TOKEN_SHOUT`.



#### Phase 2: The Parser (Syntax Analysis)
* **Goal:** Turn Tokens into an **Abstract Syntax Tree (AST)**.
* **The Go Way:** You use "Recursive Descent Parsing." You write functions like `parseStatement()` and `parseExpression()`. 
* **Structure:** If the Lexer gives you `shout`, `(`, `"Hi"`, `)`, the Parser builds a "CallExpression" node where `shout` is the function and `"Hi"` is the argument.



#### Phase 3: The Semantic Analyzer (The "Brain")
* **Goal:** Ensure the code actually makes sense.
* **The Go Way:** This is where **Type Checking** happens. If the user tries to add a `string` to an `int`, the Semantic Analyzer throws an error before the code even runs.
* **Symbol Table:** You create a "Map" of every variable and function defined so the compiler knows what is "in scope."

#### Phase 4: Intermediate Representation (IR)
* **Goal:** Translate the AST into a "Halfway" language.
* **The Go Way:** Go translates your code into **Static Single Assignment (SSA)**. It’s a simplified version of your code that looks like Assembly but is easier for a computer to optimize.
* **Why?** This allows the compiler to remove unused code and make your math faster.



#### Phase 5: The Backend (Code Generation)
* **Goal:** Turn the IR into **Machine Code** (Binary).
* **The Go Way:** Go has "Arch" folders (arm64, amd64). The compiler writes the literal 1s and 0s that the CPU executes.
* **Customization Option:** If writing Machine Code is too hard for day one, you can "Transpile" your IR into C or Go and let those compilers finish the job (this is how the first version of Go worked!).

---

### Your Immediate Next Step: The Lexer "Helpers"
To finish the Lexer we started, you need the "Walking" logic. You can't just jump to the end; you have to teach your compiler how to **read**.

**Add these three helper functions to your `Lexer` struct in `engine.go`:**

```go
// isLetter checks if a character is a-z, A-Z, or _
func isLetter(ch byte) bool {
    return 'a' <= ch && ch <= 'z' || 'A' <= ch && ch <= 'Z' || ch == '_'
}

// readIdentifier reads characters until it hits a non-letter (like a space or bracket)
func (l *Lexer) readIdentifier() string {
    position := l.position
    for isLetter(l.ch) {
        l.readChar()
    }
    return l.input[position:l.position]
}

// skipWhitespace ensures our compiler ignores spaces and tabs
func (l *Lexer) skipWhitespace() {
    for l.ch == ' ' || l.ch == '\t' || l.ch == '\n' || l.ch == '\r' {
        l.readChar()
    }
}
```

### The Challenge
Once you add these, your `NextToken()` function will actually work. You will be able to feed it `shout(name)` and it will return:
1. `IDENT` ("shout")
2. `LPAREN` ("(")
3. `IDENT` ("name")
4. `RPAREN` (")")

**Are you ready to try running this "Character-by-Character" Lexer to see if it correctly identifies your tokens?** This is Level 1 of the Roadmap!