package logger

import (
	"fmt"
	"io"
	"os"
	"sync"
	"time"
)

type Level int

const (
	DEBUG Level = iota
	INFO
	WARN
	ERROR
	FATAL
)

var levelNames = []string{
	"DEBUG",
	"INFO",
	"WARN",
	"ERROR",
	"FATAL",
}

type Logger struct {
	mu     sync.Mutex
	output io.Writer
	level  Level
	prefix string
	quiet  bool
}

var defaultLogger = &Logger{
	output: os.Stderr,
	level:  INFO,
	prefix: "gset",
}

var once sync.Once

func init() {
	if os.Getenv("GSET_DEBUG") != "" {
		defaultLogger.level = DEBUG
	}
}

func Default() *Logger {
	once.Do(func() {
		defaultLogger = &Logger{
			output: os.Stderr,
			level:  INFO,
			prefix: "gset",
		}
	})
	return defaultLogger
}

func (l *Logger) SetLevel(level Level) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.level = level
}

func (l *Logger) SetOutput(w io.Writer) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.output = w
}

func (l *Logger) SetPrefix(prefix string) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.prefix = prefix
}

func (l *Logger) SetQuiet(quiet bool) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.quiet = quiet
}

func (l *Logger) log(level Level, format string, args ...interface{}) {
	if level < l.level {
		return
	}

	l.mu.Lock()
	defer l.mu.Unlock()

	if l.quiet && level >= WARN {
		return
	}

	msg := fmt.Sprintf(format, args...)
	timestamp := time.Now().Format("2006-01-02 15:04:05")
	levelName := levelNames[level]

	if l.prefix != "" {
		fmt.Fprintf(l.output, "[%s] [%s] [%s] %s\n", timestamp, l.prefix, levelName, msg)
	} else {
		fmt.Fprintf(l.output, "[%s] [%s] %s\n", timestamp, levelName, msg)
	}
}

func (l *Logger) Debug(format string, args ...interface{}) {
	l.log(DEBUG, format, args...)
}

func (l *Logger) Info(format string, args ...interface{}) {
	l.log(INFO, format, args...)
}

func (l *Logger) Warn(format string, args ...interface{}) {
	l.log(WARN, format, args...)
}

func (l *Logger) Error(format string, args ...interface{}) {
	l.log(ERROR, format, args...)
}

func (l *Logger) Fatal(format string, args ...interface{}) {
	l.log(FATAL, format, args...)
	os.Exit(1)
}

func Debug(format string, args ...interface{}) {
	defaultLogger.log(DEBUG, format, args...)
}

func Info(format string, args ...interface{}) {
	defaultLogger.log(INFO, format, args...)
}

func Warn(format string, args ...interface{}) {
	defaultLogger.log(WARN, format, args...)
}

func Error(format string, args ...interface{}) {
	defaultLogger.log(ERROR, format, args...)
}

func Fatal(format string, args ...interface{}) {
	defaultLogger.log(FATAL, format, args...)
}

func SetLevel(level Level) {
	defaultLogger.SetLevel(level)
}

func SetOutput(w io.Writer) {
	defaultLogger.SetOutput(w)
}

func SetQuiet(quiet bool) {
	defaultLogger.SetQuiet(quiet)
}
