package call

import (
	"context"
	"crypto/tls"
	"errors"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"time"
)

const (
	// MaxVideoFileSize is the maximum accepted video download size.
	MaxVideoFileSize = 50 << 20 // 50MB
	// VideoDownloadTimeout is the HTTP client timeout for video downloads.
	VideoDownloadTimeout = 30 * time.Second
)

var (
	// ErrUnsupportedVideo indicates the downloaded file is not MP4/WebM/AVI.
	ErrUnsupportedVideo = errors.New("unsupported_video")
	// ErrVideoDownloadFailed indicates the video file could not be fetched.
	ErrVideoDownloadFailed = errors.New("video_download_failed")
)

// PrepareVideoFile downloads videoURL to a temp file, enforcing SSRF protection,
// a max size and a download timeout, and validating the container format. The
// returned VideoResult's Cleanup removes the temp file after playback.
//
// SSRF hardening mirrors PrepareAudioFile: the URL is resolved once, every
// resolved address is validated against the blocked ranges, and the connection
// is pinned (dialed directly) to a validated IP so a subsequent DNS
// re-resolution cannot reach an internal address. Redirects are rejected
// outright (a 3xx is treated as a failed download).
func PrepareVideoFile(ctx context.Context, videoURL string) (*VideoResult, error) {
	u, err := url.Parse(videoURL)
	if err != nil {
		return nil, ErrVideoDownloadFailed
	}
	// Protocol check: only http/https (SSRF protection).
	if u.Scheme != "http" && u.Scheme != "https" {
		return nil, ErrVideoDownloadFailed
	}
	host := u.Hostname()
	if host == "" {
		return nil, ErrVideoDownloadFailed
	}
	port := u.Port()
	if port == "" {
		if u.Scheme == "https" {
			port = "443"
		} else {
			port = "80"
		}
	}
	// Validate the resolved address once up front.
	ip, err := resolveAllowed(ctx, host)
	if err != nil {
		if errors.Is(err, ErrBlockedAddress) {
			return nil, ErrBlockedAddress
		}
		return nil, ErrVideoDownloadFailed
	}

	// Dial the validated IP directly (DNS pinning). The Host header / SNI still
	// carry the original hostname so TLS and virtual hosts work as expected.
	dialer := &net.Dialer{Timeout: VideoDownloadTimeout}
	transport := &http.Transport{
		DialContext: func(ctx context.Context, network, _ string) (net.Conn, error) {
			return dialer.DialContext(ctx, network, net.JoinHostPort(ip.String(), port))
		},
	}
	if u.Scheme == "https" {
		transport.TLSClientConfig = &tls.Config{
			// Preserve the original host for SNI and certificate validation while
			// dialing the pinned IP.
			ServerName: host,
		}
	}

	client := &http.Client{
		Timeout:   VideoDownloadTimeout,
		Transport: transport,
		// Disable redirects: follow nothing, treat any 3xx as a failed download.
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}
	resp, err := client.Get(videoURL)
	if err != nil {
		return nil, ErrVideoDownloadFailed
	}
	defer resp.Body.Close()
	// A 3xx means we refused to follow a redirect — treat it as a failed/blocked
	// download rather than chasing the target.
	if resp.StatusCode != http.StatusOK {
		return nil, ErrVideoDownloadFailed
	}

	tmp, err := os.CreateTemp("", "wa-video-*")
	if err != nil {
		return nil, ErrVideoDownloadFailed
	}
	path := tmp.Name()

	// Enforce max size: read up to Max+1 bytes and treat overflow as failure.
	limited := io.LimitReader(resp.Body, MaxVideoFileSize+1)
	written, err := io.Copy(tmp, limited)
	if err != nil {
		tmp.Close()
		_ = os.Remove(path)
		return nil, ErrVideoDownloadFailed
	}
	if written > MaxVideoFileSize {
		tmp.Close()
		_ = os.Remove(path)
		return nil, ErrVideoDownloadFailed
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(path)
		return nil, ErrVideoDownloadFailed
	}

	format, err := sniffVideoFormat(path)
	if err != nil {
		_ = os.Remove(path)
		return nil, err
	}

	return NewVideoResult(path, format), nil
}

// NewVideoResult wraps a temp video file path and its sniffed format in a
// VideoResult whose Cleanup removes the file.
func NewVideoResult(path, format string) *VideoResult {
	return &VideoResult{
		Path:   path,
		Format: format,
		Cleanup: func() {
			_ = os.Remove(path)
		},
	}
}

// ValidateAndWrapVideoFile sniffs the container format of an already-saved temp
// file (e.g. an upload written by the caller) and returns a VideoResult with
// cleanup, or ErrUnsupportedVideo if the format is not recognized.
func ValidateAndWrapVideoFile(tmpPath string) (*VideoResult, error) {
	format, err := sniffVideoFormat(tmpPath)
	if err != nil {
		return nil, err
	}
	return NewVideoResult(tmpPath, format), nil
}

// sniffVideoFormat inspects the file header (magic bytes) and returns "mp4",
// "webm" or "avi", or ErrUnsupportedVideo.
func sniffVideoFormat(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", ErrVideoDownloadFailed
	}
	defer f.Close()

	header := make([]byte, 16)
	n, _ := io.ReadFull(f, header)
	header = header[:n]

	// MP4/MOV: ISO base media file — "ftyp" at offset 4 (ftypisom, ftypqt, ...).
	if len(header) >= 8 && string(header[4:8]) == "ftyp" {
		return "mp4", nil
	}
	// WebM/MKV: EBML magic 0x1A 0x45 0xDF 0xA3.
	if len(header) >= 4 && header[0] == 0x1A && header[1] == 0x45 && header[2] == 0xDF && header[3] == 0xA3 {
		return "webm", nil
	}
	// AVI: "RIFF" at 0:4 and "AVI " at 8:12.
	if len(header) >= 12 && string(header[:4]) == "RIFF" && string(header[8:12]) == "AVI " {
		return "avi", nil
	}
	return "", ErrUnsupportedVideo
}
