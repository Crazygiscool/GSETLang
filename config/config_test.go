package config

import (
	"testing"
)

func TestValidateConfig(t *testing.T) {
	tests := []struct {
		name     string
		config   GSETConfig
		wantErrs int
	}{
		{
			name: "empty config",
			config: GSETConfig{
				GlobalKeywords: make(map[string]string),
				ExtKeywords:    make(map[string]string),
				Compilers:      make(map[string]CompilerConfig),
			},
			wantErrs: 0,
		},
		{
			name: "valid compiler config",
			config: GSETConfig{
				GlobalKeywords: make(map[string]string),
				ExtKeywords:    make(map[string]string),
				Compilers: map[string]CompilerConfig{
					"py": {Command: "python3", Args: ""},
					"js": {Command: "node", Args: ""},
				},
			},
			wantErrs: 0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ValidateConfig(tt.config)
			if len(result.Errors) != tt.wantErrs {
				t.Errorf("ValidateConfig() errors = %v, want %d", result.Errors, tt.wantErrs)
			}
		})
	}
}

func TestValidateCompilerConfig(t *testing.T) {
	tests := []struct {
		name    string
		ext     string
		cfg     CompilerConfig
		wantErr bool
	}{
		{
			name:    "empty command",
			ext:     "py",
			cfg:     CompilerConfig{Command: ""},
			wantErr: false,
		},
		{
			name:    "safe command",
			ext:     "py",
			cfg:     CompilerConfig{Command: "python3"},
			wantErr: false,
		},
		{
			name:    "dangerous rm command",
			ext:     "py",
			cfg:     CompilerConfig{Command: "rm -rf /"},
			wantErr: true,
		},
		{
			name:    "dangerous dd command",
			ext:     "py",
			cfg:     CompilerConfig{Command: "dd if=/dev/zero"},
			wantErr: true,
		},
		{
			name:    "path traversal",
			ext:     "py",
			cfg:     CompilerConfig{Command: "../../bin/sh"},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateCompilerConfig(tt.ext, tt.cfg)
			if (err != nil) != tt.wantErr {
				t.Errorf("validateCompilerConfig() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}
