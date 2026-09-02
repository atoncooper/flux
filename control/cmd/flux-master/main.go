// Command flux-master is the control-plane master binary: cluster management,
// job scheduling, catalog metadata, and the REST/gRPC API.
package main

import (
	"flag"
	"log"
)

const version = "0.1.0"

func main() {
	configPath := flag.String("config", "", "path to master config file")
	flag.Parse()

	if err := run(*configPath); err != nil {
		log.Fatalf("flux-master: fatal: %v", err)
	}
}

func run(configPath string) error {
	log.Printf("flux-master %s starting", version)
	if configPath != "" {
		log.Printf("config: %s", configPath)
	}
	return nil
}
