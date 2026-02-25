package usecase

import (
	"context"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"

	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/api"
	"wa-bot/internal/infrastructure/whatsapp"
)

type TokenUseCase struct {
	waClient  *whatsapp.WhatsAppClient
	apiRepo   *api.APIRepository
	stateRepo repository.UserStateRepository
	config    repository.ConfigRepository
}

func NewTokenUseCase(waClient *whatsapp.WhatsAppClient, apiRepo *api.APIRepository, stateRepo repository.UserStateRepository, config repository.ConfigRepository) *TokenUseCase {
	return &TokenUseCase{
		waClient:  waClient,
		apiRepo:   apiRepo,
		stateRepo: stateRepo,
		config:    config,
	}
}

func (uc *TokenUseCase) HandleToken(ctx context.Context, senderJID waTypes.JID, role string, isFromGroup bool) error {
	isAllowed := role == "ADMIN" || role == "OWNER" || role == "USER"

	if !isAllowed || isFromGroup {
		uc.waClient.SendMessageToJID(ctx, senderJID, "Invalid Command")
		return nil
	}

	uc.stateRepo.SetUserState(senderJID.String(), "PendingToken")
	uc.waClient.SendMessageToJID(ctx, senderJID, "Silakan masukkan nama lengkap Anda.")
	return nil
}

func (uc *TokenUseCase) HandleNameInput(ctx context.Context, senderJID waTypes.JID, messageText string) error {
	uc.waClient.SendMessageToJID(ctx, senderJID, "⏳ Loading...")

	cancelCtx, cancel := context.WithCancel(context.Background())
	uc.stateRepo.UpdateProcessContext(senderJID.String(), cancel)

	go func() {
		defer uc.stateRepo.ClearUserState(senderJID.String())
		defer cancel()

		timeoutStr := os.Getenv("TIMEOUT_NAMA")
		timeout, err := strconv.Atoi(timeoutStr)
		if err != nil {
			timeout = 2
		}

		userState, err := uc.stateRepo.GetUserStatus(senderJID.String())
		if err == nil && userState != nil {
			if time.Since(userState.StartTime) > time.Duration(timeout)*time.Minute {
				uc.waClient.SendMessageToJID(cancelCtx, senderJID, "⏳ Waktu habis! Silakan ketik *!token* lagi.")
				return
			}
		}

		validNameRegex := regexp.MustCompile(`^[a-zA-Z' ]+$`)
		if !validNameRegex.MatchString(messageText) {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "⚠️ Nama Invalid")
			return
		}

		nis := strings.Split(senderJID.String(), "@")[0]
		nama := messageText

		status, token, err := uc.apiRepo.FetchToken(cancelCtx, nama, nis)
		if err != nil {
			uc.waClient.SendMessageToJID(cancelCtx, senderJID, "Gagal mendapatkan token.")
			return
		}

		var responseText string
		if status == "new" {
			responseText = "✅ Token baru Anda adalah:"
		} else if status == "update" {
			responseText = "Token lama telah tidak berlaku. Ini token baru anda:"
		}

		uc.waClient.SendMessageToJID(cancelCtx, senderJID, responseText)
		uc.waClient.SendMessageToJID(cancelCtx, senderJID, token)
	}()

	return nil
}
