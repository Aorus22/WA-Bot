package repository

import (
	"context"

	"wa-bot/internal/domain/entity"
)

// CallHistoryFilter filters the call history listing.
type CallHistoryFilter struct {
	Limit     int
	Before    *int64
	Direction entity.CallDirection
	Type      entity.CallType
	Status    entity.CallStatus
	Target    string
}

// CallRepository persists call logs.
type CallRepository interface {
	CreateCallLog(ctx context.Context, log *entity.CallLog) error
	UpdateCallStatus(ctx context.Context, id string, status entity.CallStatus, answeredAt *int64, endedAt *int64, durationMS *int64, meowCallID string) error
	GetCallLog(ctx context.Context, id string) (*entity.CallLog, error)
	GetCallHistory(ctx context.Context, opts CallHistoryFilter) ([]*entity.CallLog, error)
	MarkInterruptedCalls(ctx context.Context) (int64, error)
}
