package entity

type WebhookLog struct {
	ID          string `json:"id"`
	WebhookID   string `json:"webhook_id"`
	WebhookPath string `json:"webhook_path"`
	SourceIP    string `json:"source_ip"`
	Method      string `json:"method"`
	Headers     string `json:"headers"`
	Body        string `json:"body"`
	QueryParams string `json:"query_params"`
	StatusCode  int    `json:"status_code"`
	CreatedAt   int64  `json:"created_at"`
}
