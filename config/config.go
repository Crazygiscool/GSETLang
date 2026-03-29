package config

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
)

var (
	ErrInvalidKey    = errors.New("invalid configuration key")
	ErrInvalidValue  = errors.New("invalid configuration value")
	ErrUnsafeCommand = errors.New("unsafe command in configuration")
)

const maxConfigSize = 1024 * 1024 // 1MB
const maxKeywords = 1000

type GSETConfig struct {
	Keywords       map[string]string
	GlobalKeywords map[string]string
	ExtKeywords    map[string]string
	Compilers      map[string]CompilerConfig
	loadedFrom     string
}

type CompilerConfig struct {
	Command string
	Args    string
	Wrapper string
	Run     string
}

type ValidationResult struct {
	Errors   []string
	Warnings []string
}

func LoadConfig(filePath string) (GSETConfig, error) {
	conf := GSETConfig{
		Keywords:       make(map[string]string),
		GlobalKeywords: make(map[string]string),
		ExtKeywords:    make(map[string]string),
		Compilers:      make(map[string]CompilerConfig),
		loadedFrom:     "",
	}

	if filePath == "" {
		execDir, _ := os.Getwd()
		filePath = execDir
	}

	dir := filepath.Dir(filePath)
	execDir, _ := os.Getwd()

	confFiles := []string{
		filepath.Join(dir, "gset.conf"),
		filepath.Join(execDir, "gset.conf"),
		filepath.Join(os.Getenv("HOME"), ".gset.conf"),
		"/etc/gset.conf",
	}

	for _, cf := range confFiles {
		if data, err := os.ReadFile(cf); err == nil {
			if len(string(data)) > maxConfigSize {
				return conf, ErrInvalidValue
			}
			conf.loadConfigFile(string(data))
			conf.loadedFrom = cf
			break
		}
	}

	return conf, nil
}

func ValidateConfig(conf GSETConfig) ValidationResult {
	result := ValidationResult{
		Errors:   []string{},
		Warnings: []string{},
	}

	if len(conf.GlobalKeywords) > maxKeywords {
		result.Errors = append(result.Errors, "too many global keywords")
	}

	if len(conf.ExtKeywords) > maxKeywords {
		result.Errors = append(result.Errors, "too many extension keywords")
	}

	for ext, cfg := range conf.Compilers {
		if err := validateCompilerConfig(ext, cfg); err != nil {
			result.Errors = append(result.Errors, err.Error())
		}
	}

	return result
}

func validateCompilerConfig(ext string, cfg CompilerConfig) error {
	if cfg.Command == "" {
		return nil
	}

	dangerousCommands := []string{"rm", "del", "format", "mkfs", "dd"}
	lowerCmd := strings.ToLower(cfg.Command)
	for _, dc := range dangerousCommands {
		if strings.Contains(lowerCmd, dc) {
			return ErrUnsafeCommand
		}
	}

	if strings.Contains(cfg.Command, "..") {
		return ErrUnsafeCommand
	}

	return nil
}

func (c *GSETConfig) Validate() ValidationResult {
	return ValidateConfig(*c)
}

func (c *GSETConfig) loadConfigFile(content string) {
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		pair := strings.SplitN(line, "=", 2)
		if len(pair) != 2 {
			continue
		}

		key := strings.TrimSpace(pair[0])
		val := strings.TrimSpace(pair[1])

		if strings.HasPrefix(key, "ext.") {
			parts := strings.Split(key, ".")
			if len(parts) >= 3 {
				ext := parts[1]
				kw := parts[2]
				c.ExtKeywords[ext+"."+kw] = val
			}
		} else if strings.HasPrefix(key, "compiler.") {
			parts := strings.Split(key, ".")
			if len(parts) >= 3 {
				ext := parts[1]
				prop := parts[2]
				if _, ok := c.Compilers[ext]; !ok {
					c.Compilers[ext] = CompilerConfig{}
				}
				cfg := c.Compilers[ext]
				switch prop {
				case "command":
					cfg.Command = val
				case "args":
					cfg.Args = val
				case "wrapper":
					cfg.Wrapper = val
				case "run":
					cfg.Run = val
				}
				c.Compilers[ext] = cfg
			}
		} else {
			c.GlobalKeywords[key] = val
		}
	}
}

func (c *GSETConfig) GetKeywords(fileKeywords map[string]string, ext string) map[string]string {
	merged := make(map[string]string)

	for k, v := range c.GlobalKeywords {
		merged[k] = v
	}

	for k, v := range c.ExtKeywords {
		extPart := strings.Split(k, ".")[0]
		if extPart == ext {
			kw := strings.SplitN(k, ".", 2)[1]
			merged[kw] = v
		}
	}

	for k, v := range fileKeywords {
		merged[k] = v
	}

	return merged
}
