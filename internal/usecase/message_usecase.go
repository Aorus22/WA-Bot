package usecase

import (
	"fmt"
	"time"

	"wa-bot/internal/domain/repository"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

type MessageUseCase struct {
	msgStore *repository.MessageStore
	waClient *whatsappInfra.WhatsAppClient
}

func NewMessageUseCase(msgStore *repository.MessageStore, waClient *whatsappInfra.WhatsAppClient) *MessageUseCase {
	return &MessageUseCase{
		msgStore: msgStore,
		waClient: waClient,
	}
}

func (uc *MessageUseCase) SendMessage(chatID, content string) error {
	// Send via WhatsApp
	// This would be handled by the existing WhatsApp infrastructure

	// Save to store
	msg := &repository.Message{
		ID:        generateMessageID(),
		ChatID:    chatID,
		From:      "me",
		To:        chatID,
		Content:   content,
		Timestamp: currentTimeMillis(),
		Status:    "sent",
		Type:      "text",
	}

	return uc.msgStore.SaveMessage(msg)
}

func (uc *MessageUseCase) GetMessages(chatID string, limit int) ([]repository.Message, error) {
	return uc.msgStore.GetMessages(chatID, limit)
}

func (uc *MessageUseCase) GetChats() ([]repository.Chat, error) {
	return uc.msgStore.GetChats()
}

func (uc *MessageUseCase) GetContacts() ([]repository.Contact, error) {
	return uc.msgStore.GetContacts()
}

func (uc *MessageUseCase) SaveIncomingMessage(msg *repository.Message) error {
	return uc.msgStore.SaveMessage(msg)
}

func (uc *MessageUseCase) MarkAsRead(chatID string) error {
	return uc.msgStore.MarkAsRead(chatID)
}

func generateMessageID() string {
	return fmt.Sprintf("msg_%d", currentTimeMillis())
}

func currentTimeMillis() int64 {
	return time.Now().UnixMilli()
}
