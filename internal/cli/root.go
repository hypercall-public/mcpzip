// https://hypercall.xyz

package cli

import "fmt"

const version = "0.1.0"

// Execute runs the CLI. This is a minimal stub that will be replaced
// with cobra commands in Task C4.
func Execute() error {
	fmt.Println("mcpzip", version)
	return nil
}
