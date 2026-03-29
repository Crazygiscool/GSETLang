package security

import (
	"errors"
	"path/filepath"
	"strings"
)

var (
	ErrPathTraversal   = errors.New("path traversal attempt detected")
	ErrInvalidInput    = errors.New("invalid input detected")
	ErrDangerousSymbol = errors.New("dangerous symbol in input")
)

const (
	MaxInputSize     = 10 * 1024 * 1024 // 10MB
	MaxStringLength  = 1024 * 1024      // 1MB
	MaxIdentifierLen = 256
	MaxDepth         = 100
)

var dangerousPatterns = []string{
	"../",
	"..\\",
	"\x00",
	"\n\x00",
	"\r\n\x00",
}

var dangerousCommands = []string{
	";",
	"&&",
	"||",
	"|",
	"`",
	"$((",
	"${",
}

func SanitizePath(path string) (string, error) {
	if strings.Contains(path, "..") {
		return "", ErrPathTraversal
	}

	absPath, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}

	return absPath, nil
}

func SanitizeString(s string) (string, error) {
	if len(s) > MaxStringLength {
		return "", ErrInvalidInput
	}

	for _, pattern := range dangerousPatterns {
		if strings.Contains(s, pattern) {
			return "", ErrDangerousSymbol
		}
	}

	s = strings.ReplaceAll(s, "\x00", "")
	s = strings.ReplaceAll(s, "\r\n", "\n")
	s = strings.ReplaceAll(s, "\r", "\n")

	return s, nil
}

func SanitizeIdentifier(name string) (string, error) {
	if len(name) > MaxIdentifierLen {
		return "", ErrInvalidInput
	}

	if name == "" {
		return "", ErrInvalidInput
	}

	validName := make([]rune, 0, len(name))
	for _, r := range name {
		if r == '_' || r == '$' || r == '?' || r == '!' {
			validName = append(validName, r)
			continue
		}
		if r >= 'a' && r <= 'z' {
			validName = append(validName, r)
			continue
		}
		if r >= 'A' && r <= 'Z' {
			validName = append(validName, r)
			continue
		}
		if r >= '0' && r <= '9' && len(validName) > 0 {
			validName = append(validName, r)
			continue
		}
	}

	if len(validName) == 0 {
		return "", ErrInvalidInput
	}

	return string(validName), nil
}

func ValidateInputSize(data string) error {
	if len(data) > MaxInputSize {
		return ErrInvalidInput
	}
	return nil
}

func CheckDepth(depth int) error {
	if depth > MaxDepth {
		return errors.New("maximum nesting depth exceeded")
	}
	return nil
}

func SanitizeCommandArg(arg string) (string, error) {
	for _, pattern := range dangerousCommands {
		if strings.Contains(arg, pattern) {
			return "", ErrDangerousSymbol
		}
	}

	arg = strings.ReplaceAll(arg, ";", "")
	arg = strings.ReplaceAll(arg, "&", "")
	arg = strings.ReplaceAll(arg, "|", "")
	arg = strings.ReplaceAll(arg, "`", "")
	arg = strings.ReplaceAll(arg, "$", "")

	return arg, nil
}

func IsSafeFileExtension(ext string) bool {
	safeExtensions := map[string]bool{
		".gset":  true,
		".py":    true,
		".js":    true,
		".go":    true,
		".java":  true,
		".rb":    true,
		".php":   true,
		".ts":    true,
		".cs":    true,
		".cpp":   true,
		".c":     true,
		".swift": true,
		".kt":    true,
		".rs":    true,
	}

	ext = strings.ToLower(ext)
	return safeExtensions[ext]
}

func SanitizeFilename(filename string) (string, error) {
	if filename == "" {
		return "", ErrInvalidInput
	}

	filename = filepath.Base(filename)

	invalidChars := []string{"/", "\\", "\x00", "\n", "\r", "\t"}
	for _, c := range invalidChars {
		filename = strings.ReplaceAll(filename, c, "")
	}

	if strings.HasPrefix(filename, ".") {
		filename = "_" + filename[1:]
	}

	if filename == "" {
		return "", ErrInvalidInput
	}

	return filename, nil
}
