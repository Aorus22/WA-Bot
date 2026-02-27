package util

import (
	"os"
	"path/filepath"
	"runtime"
)

func GetBinaryPath(binName string) string {
	// 1. Check in ./bin/ folder
	localBin := filepath.Join("bin", binName)
	if runtime.GOOS == "windows" {
		localBin += ".exe"
	}

	if _, err := os.Stat(localBin); err == nil {
		return localBin
	}

	// 2. Fallback to system PATH (bare name)
	return binName
}
