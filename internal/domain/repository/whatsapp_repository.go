package repository

import (
	"context"
	"wa-bot/internal/domain/entity"
)

type WhatsAppRepository interface {
	SendMessage(ctx context.Context, to string, text string) error
	SendImage(ctx context.Context, to string, data []byte, caption string, mediaURL string, isAutomatic bool) error
	SendVideo(ctx context.Context, to string, data []byte, caption string, mediaURL string, isAutomatic bool) error
	SendDocument(ctx context.Context, to string, data []byte, title string, mediaURL string, isAutomatic bool) error
	SendSticker(ctx context.Context, to string, data []byte, isAnimated bool, mediaURL string, isAutomatic bool) error
	DeleteMessage(ctx context.Context, to string, msgID string) error
	EditMessage(ctx context.Context, to string, msgID string, newText string) error
	ReplyMessage(ctx context.Context, to string, msgID string, text string) error
	DownloadMedia(ctx context.Context, msg *entity.Message) ([]byte, bool, error)
	UploadMedia(ctx context.Context, data []byte, mediaType string) (*entity.UploadResult, error)
	GetUserInfo(ctx context.Context, jid string) (*entity.UserInfo, error)
	GetGroupInfo(ctx context.Context, jid string) (*entity.Group, error)
	GetJoinedGroups(ctx context.Context) ([]*entity.Group, error)
	Connect() error
	Disconnect() error
	GetClient() interface{}
}
