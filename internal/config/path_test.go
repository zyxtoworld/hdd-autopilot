package config

import (
	"errors"
	"path/filepath"
	"runtime"
	"testing"
)

func TestResolveDataFilePathUsesModuleDir(t *testing.T) {
	path := ResolveDataFilePath("auth.json")
	if filepath.Base(path) != "auth.json" {
		t.Fatalf("unexpected file name: %s", path)
	}
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("expected caller info")
	}
	moduleDir, found := findNearestModuleDir(filepath.Dir(file))
	if !found {
		t.Fatal("expected module dir")
	}
	if got, want := filepath.Dir(path), moduleDir; got != want {
		t.Fatalf("expected %s, got %s", want, got)
	}
}

func TestFindRootFromRuntimeDirUsesDistParent(t *testing.T) {
	root := filepath.Clean(filepath.Join("workspace", "hdd"))
	got, ok := findRootFromRuntimeDir(filepath.Join(root, "dist"))
	if !ok {
		t.Fatal("expected dist root")
	}
	if got != root {
		t.Fatalf("expected %s, got %s", root, got)
	}
}

func TestResolveDataFilePathFallsBackToExecutableDistParent(t *testing.T) {
	originalGetwd := getwd
	originalExecutable := executable
	defer func() {
		getwd = originalGetwd
		executable = originalExecutable
	}()

	getwd = func() (string, error) {
		return filepath.Clean(filepath.Join("tmp", "outside")), nil
	}
	executable = func() (string, error) {
		return filepath.Join("workspace", "hdd", "dist", "hdd-win-x64.exe"), nil
	}

	got := ResolveDataFilePath("auth.json")
	want := filepath.Join("workspace", "hdd", "auth.json")
	if got != want {
		t.Fatalf("expected %s, got %s", want, got)
	}
}

func TestResolveDataFilePathFallsBackToRelativeNameWhenNoRootFound(t *testing.T) {
	originalGetwd := getwd
	originalExecutable := executable
	defer func() {
		getwd = originalGetwd
		executable = originalExecutable
	}()

	getwd = func() (string, error) {
		return "", errors.New("boom")
	}
	executable = func() (string, error) {
		return "", errors.New("boom")
	}

	got := ResolveDataFilePath("log/mining/system/invite-codes.txt")
	want := filepath.Clean("log/mining/system/invite-codes.txt")
	if got != want {
		t.Fatalf("expected %s, got %s", want, got)
	}
}
