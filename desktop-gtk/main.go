// desktop-gtk is the native Go/GTK4 desktop frontend for WA-Bot.
// It spawns wa-bot-backend as a subprocess and provides a libadwaita UI.
package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"syscall"

	"wa-bot-desktop/internal/backend"
	"wa-bot-desktop/internal/ui"
)

const (
	appID      = "com.alyza.wa-bot-desktop"
	appName    = "WA Bot"
	appVersion = "1.1.0"
)

func main() {
	var (
		backendPath = flag.String("backend-path", defaultBackendPath(), "Path to wa-bot-backend binary")
		userDataDir = flag.String("user-data-dir", backend.UserDataDir(), "Per-user data directory (DB + media + logs)")
		noBackend   = flag.Bool("no-backend", false, "Skip spawning backend (use for dev with externally-managed backend)")
		flagPort    = flag.Int("port", 0, "If no-backend, port the externally-managed backend listens on")
		showVersion = flag.Bool("version", false, "Print version and exit")
	)
	flag.Parse()

	if *showVersion {
		fmt.Printf("%s %s\n", appName, appVersion)
		return
	}

	log.SetFlags(log.LstdFlags | log.Lmicroseconds)
	log.Printf("Starting %s %s", appName, appVersion)
	log.Printf("User data dir: %s", *userDataDir)
	log.Printf("Backend path: %s", *backendPath)

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	var mgr *backend.Manager
	var portCh <-chan int
	if !*noBackend {
		mgr = backend.NewManager(*backendPath, *userDataDir)
		if err := mgr.Start(ctx); err != nil {
			log.Fatalf("failed to start backend: %v", err)
		}
		portCh = mgr.Port()
	} else {
		log.Printf("Running without backend manager; using port %d", *flagPort)
		portCh = portFromInt(*flagPort)
	}

	app, err := ui.NewApp(ui.AppConfig{
		ID:          appID,
		Name:        appName,
		Version:     appVersion,
		UserDataDir: *userDataDir,
		Manager:     mgr,
		PortChannel: portCh,
	})
	if err != nil {
		log.Fatalf("failed to create app: %v", err)
	}
	exitCode := app.Run(ctx)
	if exitCode != 0 {
		log.Printf("app exited with code %d", exitCode)
		os.Exit(exitCode)
	}
	log.Printf("Shutdown clean")
}

// defaultBackendPath returns the most likely location of wa-bot-backend.
// Prod: sibling to the desktop binary.
// Dev fallback: ../wa-bot-backend relative to the desktop-gtk directory.
func defaultBackendPath() string {
	exeSuffix := ""
	if runtime.GOOS == "windows" {
		exeSuffix = ".exe"
	}

	exe, err := os.Executable()
	if err == nil {
		dir := filepath.Dir(exe)
		candidate := filepath.Join(dir, "wa-bot-backend"+exeSuffix)
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		// dev: one level up (desktop-gtk/ -> repo root)
		return filepath.Join(dir, "..", "wa-bot-backend"+exeSuffix)
	}
	return "wa-bot-backend" + exeSuffix
}

// portFromInt returns a channel that immediately yields the given port and closes.
// Used when running with -no-backend to skip the subprocess handshake.
func portFromInt(p int) <-chan int {
	ch := make(chan int, 1)
	ch <- p
	close(ch)
	return ch
}
