package usecase

import (
	"context"

	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/whatsapp"
)

type HandlerUseCase struct {
	StateRepo repository.UserStateRepository
	waClient  *whatsapp.WhatsAppClient
}

func NewHandlerUseCase(stateRepo repository.UserStateRepository, waClient *whatsapp.WhatsAppClient) *HandlerUseCase {
	return &HandlerUseCase{
		StateRepo: stateRepo,
		waClient:  waClient,
	}
}

func (uc *HandlerUseCase) HandleCancel(senderJID string) {
	state := uc.StateRepo.GetUserStateSimple(senderJID)
	targetJID, _ := waTypes.ParseJID(senderJID)

	if state == "" {
		uc.waClient.SendMessageToJID(context.Background(), targetJID, "There is no running process", true)
		return
	}

	err := uc.StateRepo.CancelUserState(senderJID)
	if err != nil {
		uc.waClient.SendMessageToJID(context.Background(), targetJID, "⚠️ Failed to cancel process", true)
		return
	}

	uc.waClient.SendMessageToJID(context.Background(), targetJID, "✅ Process successfully cancelled", true)
}

type WhatsAppService struct {
	waClient *whatsapp.WhatsAppClient
	config   repository.ConfigRepository
}

func NewWhatsAppService(waClient *whatsapp.WhatsAppClient, config repository.ConfigRepository) *WhatsAppService {
	return &WhatsAppService{
		waClient: waClient,
		config:   config,
	}
}
