package usecase

import (
	"context"
	"fmt"
	"os"
	"strings"

	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/api"
	"wa-bot/internal/infrastructure/whatsapp"
)

type AdminUseCase struct {
	waClient *whatsapp.WhatsAppClient
	apiRepo  *api.APIRepository
	config   repository.ConfigRepository
}

func NewAdminUseCase(waClient *whatsapp.WhatsAppClient, apiRepo *api.APIRepository, config repository.ConfigRepository) *AdminUseCase {
	return &AdminUseCase{
		waClient: waClient,
		apiRepo:  apiRepo,
		config:   config,
	}
}

func (uc *AdminUseCase) ListGroups(ctx context.Context, senderJID string, role string) error {
	if role != "OWNER" {
		uc.waClient.SendMessage(ctx, senderJID, "Invalid Command", true)
		return nil
	}

	groups, err := uc.waClient.GetJoinedGroups(ctx)
	if err != nil {
		uc.waClient.SendMessage(ctx, senderJID, "Failed to get groups", true)
		return err
	}

	responseText := "📌 *Daftar Grup:*\n\n"
	for _, group := range groups {
		responseText += fmt.Sprintf("📂 *%s*\n📎 ID: %s\n", group.Name, group.JID)
	}

	uc.waClient.SendMessage(ctx, senderJID, responseText, true)
	return nil
}

func (uc *AdminUseCase) ListMapel(ctx context.Context, senderJID string, role string) error {
	isAllowed := role == "ADMIN" || role == "OWNER"
	if !isAllowed {
		return nil
	}

	listMapel, err := uc.apiRepo.FetchMapel()
	if err != nil {
		uc.waClient.SendMessage(ctx, senderJID, "Gagal mengambil daftar mapel.", true)
		return err
	}

	var listMapelString string
	for i, mapel := range listMapel {
		listMapelString += fmt.Sprintf("%d. %s\n", i+1, mapel)
	}

	uc.waClient.SendMessage(ctx, senderJID, listMapelString, true)
	return nil
}

func (uc *AdminUseCase) ListMembers(ctx context.Context, senderJID string, role string) error {
	if role != "OWNER" {
		uc.waClient.SendMessage(ctx, senderJID, "Invalid Command", true)
		return nil
	}

	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	responseText := "*Daftar Member per Grup:*\n\n"

	for _, userGroup := range userGroups {
		userGroup = strings.TrimSpace(userGroup)
		if userGroup == "" {
			continue
		}

		groupInfo, err := uc.waClient.GetGroupInfo(ctx, userGroup)
		if err != nil {
			fmt.Printf("Failed to get group info for '%s': %v\n", userGroup, err)
			continue
		}

		responseText += fmt.Sprintf("*%s* (%d members)\n", groupInfo.Name, len(groupInfo.Participants))
		for _, participant := range groupInfo.Participants {
			jid, err := waTypes.ParseJID(participant.JID)
			if err != nil {
				continue
			}
			responseText += fmt.Sprintf("- %s (JID: %s)\n", jid.User, jid.String())
		}
		responseText += "\n"
	}

	uc.waClient.SendMessage(ctx, senderJID, responseText, true)
	return nil
}
