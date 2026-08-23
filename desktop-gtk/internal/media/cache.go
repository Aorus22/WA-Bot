// Package media downloads backend media and avatars into an on-disk cache
// and turns image bytes into GdkTextures. Network work happens on goroutines;
// texture creation hops to the GTK main thread because GDK types are not
// thread-safe.
package media

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"

	"wa-bot-desktop/internal/api"
)

// Cache stores downloaded files under <userDataDir>/cache.
type Cache struct {
	client *api.Client
	dir    string

	mu       sync.Mutex
	inflight map[string]*call

	// Decoded-texture memory cache (URL -> GdkTexture). Written and read on
	// the GTK main thread only; the mutex is just belt-and-suspenders. This
	// lets rebuilt widgets (chat rows, bubbles) apply images synchronously
	// instead of blinking through an async reload.
	memMu sync.Mutex
	mem   map[string]*gdk.Texture
}

type call struct {
	wg   sync.WaitGroup
	path string
	err  error
}

// NewCache builds a Cache rooted at <userDataDir>/cache.
func NewCache(client *api.Client, userDataDir string) *Cache {
	dir := filepath.Join(userDataDir, "cache")
	_ = os.MkdirAll(dir, 0o755)
	return &Cache{
		client:   client,
		dir:      dir,
		inflight: make(map[string]*call),
		mem:      make(map[string]*gdk.Texture),
	}
}

// Dir returns the cache root (for debugging/clearing).
func (c *Cache) Dir() string { return c.dir }

// Clear removes every cached file (called on logout).
func (c *Cache) Clear() {
	c.mu.Lock()
	entries, err := os.ReadDir(c.dir)
	if err == nil {
		for _, e := range entries {
			_ = os.RemoveAll(filepath.Join(c.dir, e.Name()))
		}
	}
	c.mu.Unlock()

	c.memMu.Lock()
	c.mem = make(map[string]*gdk.Texture)
	c.memMu.Unlock()
}

// MemoryTexture returns the previously decoded texture for rawURL, if any.
// Call from the GTK main thread.
func (c *Cache) MemoryTexture(rawURL string) *gdk.Texture {
	c.memMu.Lock()
	defer c.memMu.Unlock()
	return c.mem[rawURL]
}

// RememberTexture stores a decoded texture under its URL so future widget
// rebuilds can apply it instantly. Call from the GTK main thread.
func (c *Cache) RememberTexture(rawURL string, tex *gdk.Texture) {
	if tex == nil {
		return
	}
	c.memMu.Lock()
	defer c.memMu.Unlock()
	if c.mem == nil {
		c.mem = make(map[string]*gdk.Texture)
	}
	c.mem[rawURL] = tex
}

// Get returns the local path for rawURL, downloading it once. Concurrent
// callers of the same URL share one download. Safe from any goroutine.
func (c *Cache) Get(ctx context.Context, rawURL string) (string, error) {
	key := cacheKey(rawURL)
	dst := filepath.Join(c.dir, key)

	if _, err := os.Stat(dst); err == nil {
		return dst, nil
	}

	c.mu.Lock()
	if cl, ok := c.inflight[key]; ok {
		c.mu.Unlock()
		cl.wg.Wait()
		return cl.path, cl.err
	}
	cl := &call{}
	cl.wg.Add(1)
	c.inflight[key] = cl
	c.mu.Unlock()

	go func() {
		defer cl.wg.Done()
		cl.path, cl.err = c.download(ctx, rawURL, dst)
		c.mu.Lock()
		delete(c.inflight, key)
		c.mu.Unlock()
	}()

	cl.wg.Wait()
	return cl.path, cl.err
}

// ImageAsync downloads rawURL off-thread and invokes done on the GTK main
// thread with the decoded texture (or error). Successful textures are stored
// in the memory cache so later widget rebuilds apply them instantly.
func (c *Cache) ImageAsync(rawURL string, done func(tex *gdk.Texture, err error)) {
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		path, err := c.Get(ctx, rawURL)
		var tex *gdk.Texture
		if err == nil {
			tex, err = TextureFromFile(path)
		}
		glib.IdleAdd(func() bool {
			if err == nil && tex != nil {
				c.RememberTexture(rawURL, tex)
			}
			done(tex, err)
			return false
		})
	}()
}

// download streams the URL to dst (temp file + rename for atomicity).
func (c *Cache) download(ctx context.Context, rawURL, dst string) (string, error) {
	data, err := c.client.FetchBytes(ctx, rawURL)
	if err != nil {
		log.Printf("media: fetch %s: %v", rawURL, err)
		return "", err
	}
	tmp := dst + ".part"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return "", fmt.Errorf("write cache: %w", err)
	}
	if err := os.Rename(tmp, dst); err != nil {
		_ = os.Remove(tmp)
		return "", fmt.Errorf("rename cache: %w", err)
	}
	return dst, nil
}

// TextureFromFile decodes any GDK-supported image into a Texture. Must be
// called on the GTK main thread.
func TextureFromFile(path string) (*gdk.Texture, error) {
	tex, err := gdk.NewTextureFromFile(gio.NewFileForPath(path))
	if err != nil {
		return nil, fmt.Errorf("decode %s: %w", filepath.Base(path), err)
	}
	return tex, nil
}

// TextureFromBytes decodes PNG/JPEG bytes into a Texture on the main thread.
func TextureFromBytes(data []byte) (*gdk.Texture, error) {
	tex, err := gdk.NewTextureFromBytes(glib.NewBytes(data))
	if err != nil {
		return nil, fmt.Errorf("decode image bytes: %w", err)
	}
	return tex, nil
}

// cacheKey derives a stable filename from the URL, preserving the original
// extension so GDK can sniff the format.
func cacheKey(rawURL string) string {
	sum := sha256.Sum256([]byte(rawURL))
	name := hex.EncodeToString(sum[:16])
	if i := strings.LastIndexByte(rawURL, '.'); i > strings.LastIndexByte(rawURL, '/') {
		ext := rawURL[i:]
		if len(ext) <= 5 && !strings.ContainsAny(ext, "?#&=") {
			name += strings.ToLower(ext)
		}
	}
	return name
}
