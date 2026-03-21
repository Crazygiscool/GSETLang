package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

var Version = "dev"

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	cmd := os.Args[1]

	switch cmd {
	case "install":
		install()
	case "uninstall":
		uninstall()
	case "version", "v":
		fmt.Println("GSET version:", Version)
	default:
		fmt.Println("Unknown command:", cmd)
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Println("GSET Installer")
	fmt.Println("")
	fmt.Println("Usage: install <command>")
	fmt.Println("")
	fmt.Println("Commands:")
	fmt.Println("  install   Install GSET to system")
	fmt.Println("  uninstall Remove GSET from system")
	fmt.Println("  version   Show version")
}

func install() {
	exePath, err := os.Executable()
	if err != nil {
		fmt.Println("Error finding executable:", err)
		os.Exit(1)
	}

	// Detect OS and install accordingly
	switch runtimeOS := os.Getenv("OSTYPE"); {
	case strings.Contains(runtimeOS, "linux"):
		installLinux(exePath)
	case strings.Contains(runtimeOS, "darwin"):
		installMac(exePath)
	case strings.Contains(runtimeOS, "win"):
		installWindows(exePath)
	default:
		fmt.Println("Unknown OS, attempting Linux install...")
		installLinux(exePath)
	}
}

func installLinux(exePath string) {
	// Try to install to /usr/local/bin first (requires sudo), fallback to ~/.local/bin
	locations := []string{"/usr/local/bin", filepath.Join(os.Getenv("HOME"), ".local", "bin")}

	for _, loc := range locations {
		dest := filepath.Join(loc, "gset")
		if _, err := os.Stat(loc); os.IsNotExist(err) {
			if err := os.MkdirAll(loc, 0755); err != nil {
				continue
			}
		}

		if err := copyFile(exePath, dest); err == nil {
			os.Chmod(dest, 0755)
			fmt.Printf("Installed to: %s\n", dest)
			return
		}
	}

	// Fallback: create alias in PATH
	fmt.Println("Could not install to system directories. Adding to PATH in ~/.bashrc:")
	fmt.Println("  alias gset='" + exePath + "'")
}

func installMac(exePath string) {
	installLinux(exePath)
}

func installWindows(exePath string) {
	// Try Program Files first, fallback to user directory
	locations := []string{
		filepath.Join(os.Getenv("ProgramFiles"), "GSET", "gset.exe"),
		filepath.Join(os.Getenv("LocalAppData"), "GSET", "gset.exe"),
	}

	for _, loc := range locations {
		dir := filepath.Dir(loc)
		if err := os.MkdirAll(dir, 0755); err != nil {
			continue
		}
		if err := copyFile(exePath, loc); err == nil {
			// Add to PATH
			addToPath(loc)
			fmt.Printf("Installed to: %s\n", loc)
			return
		}
	}

	fmt.Println("Installation failed")
}

func uninstall() {
	fmt.Println("Removing GSET...")
	// This is a basic implementation - could be expanded
}

func copyFile(src, dst string) error {
	srcFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer srcFile.Close()

	dstFile, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	buf := make([]byte, 32*1024)
	for {
		n, err := srcFile.Read(buf)
		if n > 0 {
			if _, werr := dstFile.Write(buf[:n]); werr != nil {
				return werr
			}
		}
		if err != nil {
			if err.Error() == "EOF" {
				break
			}
			return err
		}
	}
	return nil
}

func addToPath(newPath string) {
	currentPath := os.Getenv("PATH")
	if !strings.Contains(currentPath, filepath.Dir(newPath)) {
		fmt.Printf("Add this to your PATH: %s\n", filepath.Dir(newPath))
		fmt.Printf("Or run: setx PATH \"%%PATH%%;%s\"\n", filepath.Dir(newPath))
	}
}
