package security

import (
	"strings"
	"testing"
)

func TestSanitizePath(t *testing.T) {
	tests := []struct {
		input   string
		wantErr bool
	}{
		{"test.gset", false},
		{"./test.gset", false},
		{"../test.gset", true},
		{"..\\test.gset", true},
		{"/absolute/path/test.gset", false},
	}

	for _, tt := range tests {
		_, err := SanitizePath(tt.input)
		if (err != nil) != tt.wantErr {
			t.Errorf("SanitizePath(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
		}
	}
}

func TestSanitizeString(t *testing.T) {
	tests := []struct {
		input   string
		wantErr bool
	}{
		{"hello world", false},
		{"test\x00string", true},
		{"test\nstring", false},
		{"", false},
		{strings.Repeat("a", MaxStringLength+1), true},
	}

	for _, tt := range tests {
		_, err := SanitizeString(tt.input)
		if (err != nil) != tt.wantErr {
			t.Errorf("SanitizeString(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
		}
	}
}

func TestSanitizeIdentifier(t *testing.T) {
	tests := []struct {
		input   string
		wantErr bool
		wantOk  string
	}{
		{"foo", false, "foo"},
		{"_private", false, "_private"},
		{"var123", false, "var123"},
		{"foo?", false, "foo?"},
		{"foo!", false, "foo!"},
		{"", true, ""},
		{strings.Repeat("a", MaxIdentifierLen+1), true, ""},
	}

	for _, tt := range tests {
		result, err := SanitizeIdentifier(tt.input)
		if (err != nil) != tt.wantErr {
			t.Errorf("SanitizeIdentifier(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
		}
		if err == nil && result != tt.wantOk {
			t.Errorf("SanitizeIdentifier(%q) = %q, want %q", tt.input, result, tt.wantOk)
		}
	}
}

func TestValidateInputSize(t *testing.T) {
	tests := []struct {
		input string
		want  bool
	}{
		{"short input", false},
		{strings.Repeat("a", MaxInputSize), false},
		{strings.Repeat("a", MaxInputSize+1), true},
	}

	for _, tt := range tests {
		err := ValidateInputSize(tt.input)
		if (err != nil) != tt.want {
			t.Errorf("ValidateInputSize() error = %v, want %v", err, tt.want)
		}
	}
}

func TestCheckDepth(t *testing.T) {
	tests := []struct {
		depth int
		want  bool
	}{
		{0, false},
		{MaxDepth, false},
		{MaxDepth + 1, true},
		{50, false},
	}

	for _, tt := range tests {
		err := CheckDepth(tt.depth)
		if (err != nil) != tt.want {
			t.Errorf("CheckDepth(%d) error = %v, want %v", tt.depth, err, tt.want)
		}
	}
}

func TestSanitizeCommandArg(t *testing.T) {
	tests := []struct {
		input   string
		wantErr bool
		wantOk  string
	}{
		{"hello", false, "hello"},
		{"arg1;ls", true, ""},
		{"arg1 && ls", true, ""},
		{"arg1|ls", true, ""},
		{"safe_arg", false, "safe_arg"},
	}

	for _, tt := range tests {
		result, err := SanitizeCommandArg(tt.input)
		if (err != nil) != tt.wantErr {
			t.Errorf("SanitizeCommandArg(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
		}
		if err == nil && result != tt.wantOk {
			t.Errorf("SanitizeCommandArg(%q) = %q, want %q", tt.input, result, tt.wantOk)
		}
	}
}

func TestIsSafeFileExtension(t *testing.T) {
	tests := []struct {
		ext  string
		want bool
	}{
		{".gset", true},
		{".py", true},
		{".js", true},
		{".go", true},
		{".java", true},
		{".exe", false},
		{".sh", false},
		{".bat", false},
		{".ps1", false},
		{"", false},
	}

	for _, tt := range tests {
		if got := IsSafeFileExtension(tt.ext); got != tt.want {
			t.Errorf("IsSafeFileExtension(%q) = %v, want %v", tt.ext, got, tt.want)
		}
	}
}

func TestSanitizeFilename(t *testing.T) {
	tests := []struct {
		input   string
		wantErr bool
		wantOk  string
	}{
		{"test.gset", false, "test.gset"},
		{"../test.gset", false, "test.gset"},
		{".hidden", false, "_hidden"},
		{"", true, ""},
	}

	for _, tt := range tests {
		result, err := SanitizeFilename(tt.input)
		if (err != nil) != tt.wantErr {
			t.Errorf("SanitizeFilename(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
		}
		if err == nil && result != tt.wantOk {
			t.Errorf("SanitizeFilename(%q) = %q, want %q", tt.input, result, tt.wantOk)
		}
	}
}
