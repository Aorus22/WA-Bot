package api

import "context"

// AppSettings is the subset of DB-backed app settings the desktop app reads
// and writes.
type AppSettings struct {
	ReadReceipts bool
}

// GetAppSettings fetches app settings from GET /api/settings.
func (c *Client) GetAppSettings(ctx context.Context) (*AppSettings, error) {
	var out struct {
		Settings map[string]string `json:"settings"`
	}
	if err := c.doJSON(ctx, "GET", "/api/settings", nil, &out); err != nil {
		return nil, err
	}
	return parseAppSettings(out.Settings), nil
}

// UpdateAppSettings patches settings via PUT /api/settings and returns the
// refreshed values.
func (c *Client) UpdateAppSettings(ctx context.Context, changes map[string]string) (*AppSettings, error) {
	var out struct {
		Settings map[string]string `json:"settings"`
	}
	if err := c.doJSON(ctx, "PUT", "/api/settings", changes, &out); err != nil {
		return nil, err
	}
	return parseAppSettings(out.Settings), nil
}

func parseAppSettings(raw map[string]string) *AppSettings {
	s := &AppSettings{ReadReceipts: true} // WhatsApp default when unset
	if v, ok := raw["read_receipts"]; ok && v != "" {
		s.ReadReceipts = v == "true" || v == "1"
	}
	return s
}
