package whatsapp

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"time"
)

// LinkPreview carries the scraped metadata attached to outgoing
// ExtendedTextMessages so recipients render a link card.
type LinkPreview struct {
	URL           string
	Title         string
	Description   string
	JPEGThumbnail []byte
}

var previewHTTPClient = &http.Client{Timeout: 5 * time.Second}

var (
	metaTagRe = regexp.MustCompile(`(?is)<meta\s+[^>]*>`)
	attrRe    = regexp.MustCompile(`(?is)(property|name|content)\s*=\s*("([^"]*)"|'([^']*)')`)
	titleRe   = regexp.MustCompile(`(?is)<title[^>]*>(.*?)</title>`)
	entityRe  = regexp.MustCompile(`&[a-z#0-9]+;`)
)

// FirstURLInText returns the first http(s) URL found in a text, or "".
func FirstURLInText(text string) string {
	for _, field := range strings.Fields(text) {
		trimmed := strings.TrimRight(field, ".,;:!?)]}\"'")
		if strings.HasPrefix(trimmed, "http://") || strings.HasPrefix(trimmed, "https://") {
			return trimmed
		}
	}
	return ""
}

// ScrapeLinkPreview fetches a URL and extracts og:title/og:description/
// og:image (falling back to <title>/<meta description>). Best-effort: any
// failure returns nil so callers can fall back to a plain text message.
func ScrapeLinkPreview(ctx context.Context, rawURL string) *LinkPreview {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, rawURL, nil)
	if err != nil {
		return nil
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36")
	req.Header.Set("Accept", "text/html,application/xhtml+xml")
	resp, err := previewHTTPClient.Do(req)
	if err != nil {
		return nil
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		return nil
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, 768<<10))
	if err != nil {
		return nil
	}
	html := string(body)

	title := metaContent(html, "og:title")
	if title == "" {
		if m := titleRe.FindStringSubmatch(html); len(m) > 1 {
			title = decodeHTMLEntities(strings.TrimSpace(m[1]))
		}
	}
	description := metaContent(html, "og:description")
	if description == "" {
		description = metaContent(html, "description")
	}
	image := metaContent(html, "og:image")
	if title == "" && description == "" && image == "" {
		return nil
	}

	preview := &LinkPreview{URL: rawURL, Title: title, Description: description}
	if image != "" {
		if abs := resolveURL(rawURL, image); abs != "" {
			preview.JPEGThumbnail = downloadPreviewImage(ctx, abs)
		}
	}
	return preview
}

func metaContent(html, key string) string {
	for _, tag := range metaTagRe.FindAllString(html, -1) {
		var name, content string
		for _, attr := range attrRe.FindAllStringSubmatch(tag, -1) {
			value := attr[3]
			if value == "" {
				value = attr[4]
			}
			switch strings.ToLower(attr[1]) {
			case "property", "name":
				name = strings.ToLower(value)
			case "content":
				content = value
			}
		}
		if name == key && content != "" {
			return decodeHTMLEntities(strings.TrimSpace(content))
		}
	}
	return ""
}

func resolveURL(base, ref string) string {
	baseURL, err := url.Parse(base)
	if err != nil {
		return ""
	}
	refURL, err := url.Parse(ref)
	if err != nil {
		return ""
	}
	return baseURL.ResolveReference(refURL).String()
}

func downloadPreviewImage(ctx context.Context, imageURL string) []byte {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, imageURL, nil)
	if err != nil {
		return nil
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36")
	resp, err := previewHTTPClient.Do(req)
	if err != nil {
		return nil
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		return nil
	}
	data, err := io.ReadAll(io.LimitReader(resp.Body, 512<<10))
	if err != nil {
		return nil
	}
	if ct := resp.Header.Get("Content-Type"); ct != "" && !strings.HasPrefix(ct, "image/") {
		return nil
	}
	return data
}

var htmlEntities = map[string]string{
	"&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot": "\"", "&quot;": "\"",
	"&#39;": "'", "&apos;": "'", "&nbsp;": " ", "&#x27;": "'", "&#x2F;": "/",
}

func decodeHTMLEntities(s string) string {
	decoded := entityRe.ReplaceAllStringFunc(s, func(e string) string {
		if repl, ok := htmlEntities[e]; ok {
			return repl
		}
		if strings.HasPrefix(e, "&#") && strings.HasSuffix(e, ";") {
			var code int
			if _, err := fmt.Sscanf(e, "&#%d;", &code); err == nil && code > 0 && code < 0x110000 {
				return string(rune(code))
			}
		}
		return e
	})
	return strings.TrimSpace(decoded)
}
