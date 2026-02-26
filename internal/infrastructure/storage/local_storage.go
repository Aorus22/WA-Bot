package storage

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"wa-bot/internal/domain/repository"
)

type LocalStorage struct {
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
	if filepath.IsAbs(path) {
		return path
	}
	// If path already has basePath as prefix, return it as is or clean it
	// But to be safe and simple, we assume 'path' is the relative key.
	return filepath.Join(l.basePath, path)
}

func (l *LocalStorage) Exists(path string) bool {
	fullPath := l.GetPath(path)
	_, err := os.Stat(fullPath)
	return !os.IsNotExist(err)
}
