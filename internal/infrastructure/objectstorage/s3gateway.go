package objectstorage

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"hash"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/johannesboyne/gofakes3"
)

// GatewayConfig configures the optional local S3 endpoint.
type GatewayConfig struct {
	// Bind is "host:port"; the host must be a loopback address. The gateway never
	// opens itself to the network on its own.
	Bind            string
	AccessKeyID     string
	SecretAccessKey string
	Region          string // default "us-east-1"
}

// Gateway serves a SQLiteStore over the S3 HTTP protocol, inside the WA-Bot
// process. Every request must be SigV4-signed (header or presigned URL) with
// the configured key; there is no anonymous access.
type Gateway struct {
	srv  *http.Server
	ln   net.Listener
	done chan struct{}
}

// StartGateway starts serving. The returned Gateway must be Closed.
func StartGateway(store gatewayStore, bucket string, cfg GatewayConfig) (*Gateway, error) {
	if cfg.AccessKeyID == "" || len(cfg.SecretAccessKey) < 8 {
		return nil, errors.New("objectstorage: S3 gateway needs an access key and a secret of at least 8 characters")
	}
	if cfg.Region == "" {
		cfg.Region = "us-east-1"
	}
	if bucket == "" {
		bucket = DefaultBucket
	}
	host, _, err := net.SplitHostPort(cfg.Bind)
	if err != nil {
		return nil, fmt.Errorf("objectstorage: S3 gateway bind %q: %w", cfg.Bind, err)
	}
	if !isLoopbackHost(host) {
		return nil, fmt.Errorf("objectstorage: S3 gateway bind %q is not a loopback address; refusing to expose media to the network", cfg.Bind)
	}
	ln, err := net.Listen("tcp", cfg.Bind)
	if err != nil {
		return nil, err
	}

	backend := newGatewayBackend(store, bucket)
	s3 := gofakes3.New(backend,
		gofakes3.WithoutVersioning(),
		gofakes3.WithIntegrityCheck(true),
		gofakes3.WithTimeSkewLimit(maxClockSkew),
	)
	verifier := newSigV4Verifier(cfg.AccessKeyID, cfg.SecretAccessKey, cfg.Region)
	handler := http.Handler(s3.Server())
	authed := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if err := verifier.Verify(r); err != nil {
			writeS3Error(w, r, http.StatusForbidden, "AccessDenied", "Access Denied")
			return
		}
		// A signed payload hash is only worth something if the body is checked
		// against it: a captured request must not carry different content.
		if h := r.Header.Get("X-Amz-Content-Sha256"); isHexSHA256(h) && r.Body != nil {
			r.Body = &hashCheckBody{body: r.Body, want: strings.ToLower(h), sum: sha256.New()}
		}
		handler.ServeHTTP(w, r)
	})

	g := &Gateway{
		ln:   ln,
		done: make(chan struct{}),
		srv: &http.Server{
			Handler:           authed,
			ReadHeaderTimeout: 10 * time.Second,
			IdleTimeout:       2 * time.Minute,
		},
	}
	go func() {
		defer close(g.done)
		if err := g.srv.Serve(ln); err != nil && !errors.Is(err, http.ErrServerClosed) {
			fmt.Printf("[S3 GATEWAY] stopped: %v\n", err)
		}
	}()
	return g, nil
}

// Addr is the address the gateway listens on.
func (g *Gateway) Addr() string { return g.ln.Addr().String() }

// Close stops accepting requests and waits briefly for running ones.
func (g *Gateway) Close() error {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	err := g.srv.Shutdown(ctx)
	if err != nil {
		_ = g.srv.Close()
	}
	<-g.done
	return err
}

func isHexSHA256(s string) bool {
	if len(s) != 64 {
		return false
	}
	_, err := hex.DecodeString(s)
	return err == nil
}

var errPayloadHashMismatch = errors.New("the request body does not match its signed x-amz-content-sha256")

// hashCheckBody fails the read at EOF when the body does not hash to the value
// the signature covers, so a mismatching upload is aborted before it is published.
type hashCheckBody struct {
	body io.ReadCloser
	want string
	sum  hash.Hash
}

func (h *hashCheckBody) Read(p []byte) (int, error) {
	n, err := h.body.Read(p)
	h.sum.Write(p[:n])
	if err == io.EOF && hex.EncodeToString(h.sum.Sum(nil)) != h.want {
		return n, errPayloadHashMismatch
	}
	return n, err
}

func (h *hashCheckBody) Close() error { return h.body.Close() }

func isLoopbackHost(host string) bool {
	if strings.EqualFold(host, "localhost") {
		return true
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}

func writeS3Error(w http.ResponseWriter, r *http.Request, status int, code, message string) {
	w.Header().Set("Content-Type", "application/xml")
	w.WriteHeader(status)
	if r.Method == http.MethodHead {
		return
	}
	fmt.Fprintf(w, `<?xml version="1.0" encoding="UTF-8"?><Error><Code>%s</Code><Message>%s</Message></Error>`, code, message)
}
