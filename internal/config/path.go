package config

import (
	"os"
	"path/filepath"
)

var (
	getwd      = os.Getwd
	executable = os.Executable
)

func ResolveDataFilePath(name string) string {
	if root, ok := findRootFromWorkingDir(); ok {
		return filepath.Join(root, name)
	}
	if root, ok := findRootFromExecutable(); ok {
		return filepath.Join(root, name)
	}
	return filepath.Clean(name)
}

func findRootFromWorkingDir() (string, bool) {
	wd, err := getwd()
	if err != nil {
		return "", false
	}
	return findRootFromRuntimeDir(wd)
}

func findRootFromExecutable() (string, bool) {
	exePath, err := executable()
	if err != nil {
		return "", false
	}
	return findRootFromRuntimeDir(filepath.Dir(exePath))
}

func findRootFromRuntimeDir(start string) (string, bool) {
	if root, ok := findModuleDir(start); ok {
		return root, true
	}
	current := filepath.Clean(start)
	if filepath.Base(current) == "dist" {
		return filepath.Dir(current), true
	}
	return "", false
}

func findModuleDir(start string) (string, bool) {
	moduleDir, ok := findNearestModuleDir(start)
	if !ok {
		return "", false
	}
	return moduleDir, true
}

func findNearestModuleDir(start string) (string, bool) {
	current := filepath.Clean(start)
	for {
		if fileExists(filepath.Join(current, "go.mod")) {
			return current, true
		}
		parent := filepath.Dir(current)
		if parent == current {
			return "", false
		}
		current = parent
	}
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	return !info.IsDir()
}
