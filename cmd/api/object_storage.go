package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"wa-bot/internal/infrastructure/objectstorage"
	"wa-bot/internal/infrastructure/storage"
)

func envOr(key, fallback string) string {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		return v
	}
	return fallback
}

func envBool(key string) bool {
	switch strings.ToLower(strings.TrimSpace(os.Getenv(key))) {
	case "1", "true", "yes", "on":
		return true
	}
	return false
}

// objectStorageConfigFromEnv reads the provider settings:
//
//	OBJECT_STORAGE_DRIVER        sqlite (default) | s3
//	OBJECT_STORAGE_BUCKET        logical bucket, default wa-bot
//	OBJECT_STORAGE_SQLITE_PATH   default accounts/{account}/database/objectstorage.sqlite
//	                             ({account} = one database per account; a path
//	                             without it is one shared file)
//	OBJECT_STORAGE_PENDING_SQLITE_PATH  throw-away database for media seen before
//	                             login, default tmp/pending-objectstorage.sqlite
//	S3_ENDPOINT, S3_REGION, S3_ACCESS_KEY_ID, S3_SECRET_ACCESS_KEY,
//	S3_BUCKET (remote bucket, default = OBJECT_STORAGE_BUCKET), S3_FORCE_PATH_STYLE
func objectStorageConfigFromEnv() objectstorage.Config {
	bucket := envOr("OBJECT_STORAGE_BUCKET", objectstorage.DefaultBucket)
	return objectstorage.Config{
		Driver:            envOr("OBJECT_STORAGE_DRIVER", "sqlite"),
		Bucket:            bucket,
		SQLitePath:        envOr("OBJECT_STORAGE_SQLITE_PATH", filepath.Join(accountsRoot, objectstorage.AccountPlaceholder, "database", "objectstorage.sqlite")),
		PendingSQLitePath: envOr("OBJECT_STORAGE_PENDING_SQLITE_PATH", filepath.Join("tmp", "pending-objectstorage.sqlite")),
		S3: objectstorage.S3Config{
			Endpoint:        os.Getenv("S3_ENDPOINT"),
			Region:          envOr("S3_REGION", "us-east-1"),
			AccessKeyID:     os.Getenv("S3_ACCESS_KEY_ID"),
			SecretAccessKey: os.Getenv("S3_SECRET_ACCESS_KEY"),
			ForcePathStyle:  envBool("S3_FORCE_PATH_STYLE"),
			RemoteBucket:    envOr("S3_BUCKET", bucket),
		},
	}
}

// mediaRuntime is the object storage as the app uses it.
type mediaRuntime struct {
	wrapper *objectstorage.Wrapper
	media   *storage.ObjectMediaStorage
	gateway *objectstorage.Gateway
}

// setupObjectStorage opens the configured provider, prepares the scratch folder
// for external tools and, when asked, the loopback S3 endpoint.
func setupObjectStorage() (*mediaRuntime, error) {
	cfg := objectStorageConfigFromEnv()
	wrapper, err := objectstorage.New(cfg)
	if err != nil {
		return nil, fmt.Errorf("object storage (%s): %w", cfg.Driver, err)
	}
	staging, err := objectstorage.NewStaging(wrapper, envOr("OBJECT_STORAGE_STAGING_DIR", filepath.Join("tmp", "staging")))
	if err != nil {
		wrapper.Close()
		return nil, err
	}
	rt := &mediaRuntime{wrapper: wrapper, media: storage.NewObjectMediaStorage(wrapper, staging)}

	if envBool("S3_GATEWAY_ENABLED") {
		gw, err := wrapper.StartGateway(objectstorage.GatewayConfig{
			Bind:            envOr("S3_GATEWAY_BIND", "127.0.0.1:9000"),
			AccessKeyID:     os.Getenv("S3_GATEWAY_ACCESS_KEY_ID"),
			SecretAccessKey: os.Getenv("S3_GATEWAY_SECRET_ACCESS_KEY"),
			Region:          envOr("S3_GATEWAY_REGION", "us-east-1"),
		})
		if err != nil {
			wrapper.Close()
			return nil, fmt.Errorf("S3 gateway: %w", err)
		}
		rt.gateway = gw
		fmt.Printf("[STORAGE] S3 gateway on http://%s (bucket %q)\n", gw.Addr(), wrapper.Bucket())
	}

	// Scratch files that are safely stored are dropped once idle.
	go func() {
		for range time.Tick(5 * time.Minute) {
			rt.media.Sweep(15 * time.Minute)
		}
	}()
	return rt, nil
}

// close stops the gateway and closes the provider (folding the SQLite WAL back
// into the single file).
func (rt *mediaRuntime) close() {
	if rt.gateway != nil {
		_ = rt.gateway.Close()
	}
	_ = rt.wrapper.Close()
}

// closeOnSignal shuts storage down cleanly on SIGINT/SIGTERM and then exits.
func (rt *mediaRuntime) closeOnSignal() {
	ch := make(chan os.Signal, 1)
	signal.Notify(ch, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-ch
		fmt.Println("[STORAGE] shutting down")
		ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
		defer cancel()
		_ = rt.media.WorkFlushAll(ctx)
		rt.close()
		os.Exit(0)
	}()
}
