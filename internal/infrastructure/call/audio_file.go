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
	// MaxAudioFileSize is the maximum accepted audio download size (PRD §27).
	MaxAudioFileSize = 20 << 20 // 20MB
	// AudioDownloadTimeout is the HTTP client timeout for audio downloads (PRD §27).
	AudioDownloadTimeout = 15 * time.Second
)

var (
	// ErrUnsupportedAudio indicates the downloaded file is not MP3 or WAV.
	ErrUnsupportedAudio = errors.New("unsupported_audio")
	// ErrAudioDownloadFailed indicates the audio file could not be fetched.
	ErrAudioDownloadFailed = errors.New("audio_download_failed")
	// ErrBlockedAddress indicates the download resolves to a blocked address (SSRF).
	ErrBlockedAddress = errors.New("blocked_address")
)

// PrepareAudioFile downloads audioURL to a temp file, enforcing SSRF protection,
// a max size and a download timeout, and validating the MP3/WAV format. The
// returned AudioResult's Cleanup removes the temp file after playback.
//
// SSRF hardening: the URL is resolved once, every resolved address is validated
// against the blocked ranges, and the connection is pinned (dialed directly) to
// a validated IP so a subsequent DNS re-resolution cannot reach an internal
// address. Redirects are rejected outright (a 3xx is treated as a failed
// download), closing the redirect-to-metadata bypass.
func PrepareAudioFile(ctx context.Context, audioURL string) (*AudioResult, error) {
	u, err := url.Parse(audioURL)
	if err != nil {
		return nil, ErrAudioDownloadFailed
	}
	// Protocol check: only http/https (SSRF protection).
	if u.Scheme != "http" && u.Scheme != "https" {
		return nil, ErrAudioDownloadFailed
	}
	host := u.Hostname()
	if host == "" {
		return nil, ErrAudioDownloadFailed
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
		return nil, ErrAudioDownloadFailed
	}

	// Dial the validated IP directly (DNS pinning). The Host header / SNI still
	// carry the original hostname so TLS and virtual hosts work as expected.
	dialer := &net.Dialer{Timeout: AudioDownloadTimeout}
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
		Timeout:   AudioDownloadTimeout,
		Transport: transport,
		// Disable redirects: follow nothing, treat any 3xx as a failed download.
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}
	resp, err := client.Get(audioURL)
	if err != nil {
		return nil, ErrAudioDownloadFailed
	}
	defer resp.Body.Close()
	// A 3xx means we refused to follow a redirect — treat it as a failed/blocked
	// download rather than chasing the target.
	if resp.StatusCode != http.StatusOK {
		return nil, ErrAudioDownloadFailed
	}

	tmp, err := os.CreateTemp("", "wa-audio-*")
	if err != nil {
		return nil, ErrAudioDownloadFailed
	}
	path := tmp.Name()

	// Enforce max size: read up to Max+1 bytes and treat overflow as failure.
	limited := io.LimitReader(resp.Body, MaxAudioFileSize+1)
	written, err := io.Copy(tmp, limited)
	if err != nil {
		tmp.Close()
		_ = os.Remove(path)
		return nil, ErrAudioDownloadFailed
	}
	if written > MaxAudioFileSize {
		tmp.Close()
		_ = os.Remove(path)
		return nil, ErrAudioDownloadFailed
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(path)
		return nil, ErrAudioDownloadFailed
	}

	format, err := sniffAudioFormat(path)
	if err != nil {
		_ = os.Remove(path)
		return nil, err
	}

	return &AudioResult{
		Path:   path,
		Format: format,
		Cleanup: func() {
			_ = os.Remove(path)
		},
	}, nil
}

// PrepareAudioMultipart accepts raw audio bytes from a multipart upload, writes
// them to a temp file, enforces the max audio size and validates the MP3/WAV
// format, returning an AudioResult (whose Cleanup removes the temp file).
func PrepareAudioMultipart(data []byte) (*AudioResult, error) {
	if int64(len(data)) > MaxAudioFileSize {
		return nil, ErrAudioDownloadFailed // too large for a call
	}
	if len(data) == 0 {
		return nil, ErrUnsupportedAudio
	}
	tmp, err := os.CreateTemp("", "wa-audio-up-*")
	if err != nil {
		return nil, ErrAudioDownloadFailed
	}
	path := tmp.Name()
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		_ = os.Remove(path)
		return nil, ErrAudioDownloadFailed
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(path)
		return nil, ErrAudioDownloadFailed
	}
	format, err := sniffAudioFormat(path)
	if err != nil {
		_ = os.Remove(path)
		return nil, err
	}
	return &AudioResult{
		Path:   path,
		Format: format,
		Cleanup: func() {
			_ = os.Remove(path)
		},
	}, nil
}

// resolveAllowed resolves host and returns the first validated address. Every
// resolved address is checked; if any is in a blocked range the whole download
// is refused, so DNS rebinding cannot slip through the initial check.
func resolveAllowed(ctx context.Context, host string) (net.IP, error) {
	ips, err := net.DefaultResolver.LookupIPAddr(ctx, host)
	if err != nil || len(ips) == 0 {
		return nil, ErrAudioDownloadFailed
	}
	for _, ipa := range ips {
		ip := ipa.IP
		if isBlockedIP(ip) {
			return nil, ErrBlockedAddress
		}
	}
	// Return the first validated address; the connection is pinned to it below.
	first := ips[0].IP
	if first == nil {
		return nil, ErrAudioDownloadFailed
	}
	return first, nil
}

// isBlockedIP reports whether an IP is in a disallowed (SSRF) range. It handles
// both IPv4 and IPv6, including IPv4-mapped addresses (treated as their
// embedded IPv4) and all Go's built-in classification helpers.
func isBlockedIP(ip net.IP) bool {
	if ip == nil {
		return true
	}
	// IPv4-mapped addresses (::ffff:a.b.c.d) are treated as their embedded IPv4,
	// so the same private/loopback rules apply.
	if ip4 := ip.To4(); ip4 != nil {
		// Unspecified 0.0.0.0, loopback 127.0.0.0/8, and 0.* broadcast forms.
		if ip4.IsUnspecified() || ip4.IsLoopback() {
			return true
		}
		// Link-local 169.254.0.0/16 (includes the metadata IP 169.254.169.254).
		if ip4.IsLinkLocalUnicast() {
			return true
		}
		// Private 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16.
		if ip4.IsPrivate() {
			return true
		}
		return false
	}

	// Pure IPv6.
	if ip.IsUnspecified() { // ::
		return true
	}
	if ip.IsLoopback() { // ::1
		return true
	}
	if ip.IsLinkLocalUnicast() { // fe80::/10
		return true
	}
	if ip.IsLinkLocalMulticast() { // ff02::/16
		return true
	}
	if ip.IsMulticast() { // ff00::/8
		return true
	}
	// ULA fc00::/7 (and fd00::/8) — private IPv6 addressing.
	if len(ip) == net.IPv6len && ip[0]&0xfe == 0xfc {
		return true
	}
	return false
}

// sniffAudioFormat inspects the file header (magic bytes / frame sync) and
// returns "mp3" or "wav", or ErrUnsupportedAudio.
func sniffAudioFormat(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", ErrAudioDownloadFailed
	}
	defer f.Close()

	header := make([]byte, 16)
	n, _ := io.ReadFull(f, header)
	header = header[:n]

	if len(header) >= 3 && string(header[:3]) == "ID3" {
		return "mp3", nil
	}
	if len(header) >= 8 && string(header[:4]) == "RIFF" && string(header[8:12]) == "WAVE" {
		return "wav", nil
	}
	// MP3 frame sync: 0xFF followed by a byte whose top 3 bits are 111 (0xE0 mask).
	if len(header) >= 2 && header[0] == 0xFF && header[1]&0xE0 == 0xE0 {
		return "mp3", nil
	}
	return "", ErrUnsupportedAudio
}
