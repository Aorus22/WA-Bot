package storage

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"wa-bot/internal/domain/repository"
)

type LocalStorage struct {
	mu       sync.RWMutex
	basePath string
}

func NewLocalStorage(basePath string) repository.StorageRepository {
	// Create base directory if it doesn't exist
	if _, err := os.Stat(basePath); os.IsNotExist(err) {
		os.MkdirAll(basePath, 0755)
	}
	return &LocalStorage{
		basePath: basePath,
	}
}

// SetBasePath moves storage to another directory (one per WhatsApp account).
func (l *LocalStorage) SetBasePath(basePath string) error {
	if err := os.MkdirAll(basePath, 0755); err != nil {
		return err
	}
	l.mu.Lock()
	l.basePath = basePath
	l.mu.Unlock()
	return nil
}

func (l *LocalStorage) base() string {
	l.mu.RLock()
	defer l.mu.RUnlock()
	return l.basePath
}

func (l *LocalStorage) Save(ctx context.Context, path string, reader io.Reader) (string, error) {
	fullPath := l.GetPath(path)

	// Ensure directory exists
	dir := filepath.Dir(fullPath)
	if _, err := os.Stat(dir); os.IsNotExist(err) {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return "", fmt.Errorf("failed to create directory: %w", err)
		}
	}

	file, err := os.Create(fullPath)
	if err != nil {
		return "", fmt.Errorf("failed to create file: %w", err)
	}
	defer file.Close()

	_, err = io.Copy(file, reader)
	if err != nil {
		return "", fmt.Errorf("failed to save file: %w", err)
	}

	return path, nil
}

func (l *LocalStorage) Get(ctx context.Context, path string) (io.ReadCloser, error) {
	fullPath := l.GetPath(path)
	file, err := os.Open(fullPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open file %s: %w", fullPath, err)
	}
	return file, nil
}

func (l *LocalStorage) Delete(ctx context.Context, path string) error {
	fullPath := l.GetPath(path)
	err := os.Remove(fullPath)
	if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to delete file: %w", err)
	}
	return nil
}

func (l *LocalStorage) GetPath(path string) string {
	// 1. Get absolute paths for comparison
	base := l.base()
	absBase, _ := filepath.Abs(base)

	// Try to see if the path is already absolute or relative to the current working directory
	var absTarget string
	if filepath.IsAbs(path) {
		absTarget = path
	} else {
		// Join with current working directory to get an absolute path for comparison
		cwd, _ := os.Getwd()
		absTarget = filepath.Join(cwd, path)
	}

	// 2. Check if the target is already inside our basePath
	rel, err := filepath.Rel(absBase, absTarget)
	if err == nil && !strings.HasPrefix(rel, "..") && !filepath.IsAbs(rel) {
		// It's already inside the storage directory, return as is (but cleaned)
		return filepath.Clean(absTarget)
	}

	// 3. Otherwise, join it with basePath. Legacy callers pass paths prefixed
	// with the old fixed "media/" directory; the base is now per account.
	path = strings.TrimPrefix(filepath.ToSlash(path), "media/")
	fullPath := filepath.Join(base, path)

	// 4. Verify the final result is still inside for security
	rel, err = filepath.Rel(base, fullPath)
	if err != nil || strings.HasPrefix(rel, "..") || filepath.IsAbs(rel) {
		return filepath.Join(base, filepath.Base(path))
	}

	return fullPath
}

func (l *LocalStorage) Exists(path string) bool {
	fullPath := l.GetPath(path)
	_, err := os.Stat(fullPath)
	return !os.IsNotExist(err)
}
