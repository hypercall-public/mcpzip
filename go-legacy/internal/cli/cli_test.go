// https://hypercall.xyz

package cli

import (
	"os"
	"testing"
)

func TestExecute_NoArgs(t *testing.T) {
	// Save original os.Args and restore after test.
	origArgs := os.Args
	defer func() { os.Args = origArgs }()

	os.Args = []string{"mcpzip"}
	err := Execute()
	if err != nil {
		t.Errorf("Execute() with no args should not error, got: %v", err)
	}
}

func TestExecute_Version(t *testing.T) {
	origArgs := os.Args
	defer func() { os.Args = origArgs }()

	os.Args = []string{"mcpzip", "version"}
	err := Execute()
	if err != nil {
		t.Errorf("Execute() version should not error, got: %v", err)
	}
}

func TestExecute_UnknownCommand(t *testing.T) {
	origArgs := os.Args
	defer func() { os.Args = origArgs }()

	os.Args = []string{"mcpzip", "bogus"}
	err := Execute()
	if err == nil {
		t.Error("Execute() with unknown command should return error")
	}
}

func TestExecute_Init(t *testing.T) {
	origArgs := os.Args
	defer func() { os.Args = origArgs }()

	os.Args = []string{"mcpzip", "init"}
	err := Execute()
	if err != nil {
		t.Errorf("Execute() init should not error, got: %v", err)
	}
}
