package main
import "fmt"

func pick(n) {
    if (n > 0) {
        return 1
    }
    fmt.Println(pick(5))
}