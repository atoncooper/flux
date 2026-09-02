// Command flux is the Flux CLI entry point (single-machine and cluster mode).
//
// NOTE: command wiring intentionally uses only the standard library until the
// CLI framework decision (TD-004, cobra vs stdlib) is settled.
package main

import (
	"fmt"
	"os"
)

const version = "0.1.0"

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, "flux:", err)
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 {
		usage()
		return nil
	}
	switch args[0] {
	case "version":
		fmt.Println("flux", version)
	default:
		return fmt.Errorf("unknown command %q", args[0])
	}
	return nil
}

func usage() {
	fmt.Fprintln(os.Stderr, `usage: flux <command> [flags]

commands:
  version    print the flux CLI version`)
}
