package usecase

import (
	"context"
	"fmt"
	"strings"

	waTypes "go.mau.fi/whatsmeow/types"

	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/whatsapp"
)

type HandlerUseCase struct {
	StickerUC *StickerUseCase
	PDFUC     *PDFUseCase
	TokenUC   *TokenUseCase
	AdminUC   *AdminUseCase
	WaService *WhatsAppService
	StateRepo repository.UserStateRepository
	waClient  *whatsapp.WhatsAppClient
}

func NewHandlerUseCase(stickerUC *StickerUseCase, pdfUC *PDFUseCase, tokenUC *TokenUseCase, adminUC *AdminUseCase, waService *WhatsAppService, stateRepo repository.UserStateRepository, waClient *whatsapp.WhatsAppClient) *HandlerUseCase {
	return &HandlerUseCase{
		StickerUC: stickerUC,
		PDFUC:     pdfUC,
		TokenUC:   tokenUC,
		AdminUC:   adminUC,
		WaService: waService,
		StateRepo: stateRepo,
		waClient:  waClient,
	}
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

func (uc *HandlerUseCase) HandleCheck(ctx interface{}, senderJID waTypes.JID, args map[string]interface{}) {
	ctxTyped := ctx.(context.Context)
	uc.waClient.SendMessageToJID(ctxTyped, senderJID, "Hello, World!")
}

func (uc *HandlerUseCase) HandleListGroups(ctx interface{}, senderJID waTypes.JID, role string) {
	ctxTyped := ctx.(context.Context)
	uc.AdminUC.ListGroups(ctxTyped, senderJID.String(), role)
}

func (uc *HandlerUseCase) HandleToken(ctx interface{}, senderJID waTypes.JID, role string, isFromGroup bool) {
	ctxTyped := ctx.(context.Context)
	uc.TokenUC.HandleToken(ctxTyped, senderJID, role, isFromGroup)
}

func (uc *HandlerUseCase) HandleListMapel(ctx interface{}, senderJID waTypes.JID, role string) {
	ctxTyped := ctx.(context.Context)
	uc.AdminUC.ListMapel(ctxTyped, senderJID.String(), role)
}

func (uc *HandlerUseCase) HandleListMember(ctx interface{}, senderJID waTypes.JID, role string) {
	ctxTyped := ctx.(context.Context)
	uc.AdminUC.ListMembers(ctxTyped, senderJID.String(), role)
}

func (uc *HandlerUseCase) HandlePDF(ctx interface{}, senderJID waTypes.JID, messageText string, role string, msg *entity.Message) {
	ctxTyped := ctx.(context.Context)
	uc.PDFUC.SendPDF(ctxTyped, senderJID, messageText, role, msg)
}

func (uc *HandlerUseCase) HandleSticker(ctx interface{}, senderJID waTypes.JID, messageText string, role string, msg *entity.Message) {
	ctxTyped := ctx.(context.Context)
	uc.StickerUC.ConvertToSticker(ctxTyped, senderJID, messageText, role, msg)
}

func (uc *HandlerUseCase) HandleHelp(ctx interface{}, senderJID waTypes.JID, role string, args map[string]interface{}) {
	ctxTyped := ctx.(context.Context)
	var message string

	switch role {
	case "COMMON":
		message = strings.TrimSpace(`
			COMMANDS LIST

			_From URL:_
			- ` + "`!sticker`" + ` <video/gif/image URL>

			_Send with image/video/gif:_
			- ` + "`!sticker`" + `

			_Optional parameters_ (can be added after the command or URL):
			- ` + "`nocrop`" + ` // Prevent auto-cropping to square
			- ` + "`start=MM:SS`" + ` // Start time for video/gif
			- ` + "`end=MM:SS`" + ` // End time for video/gif
			- ` + "`fps=N`" + ` // Frame per second (1-60)
			- ` + "`quality=N`" + ` // Output quality (1-100)
			- ` + "`direction=side`" + ` // Pan direction: up, down, left, right
			- ` + "`direction=side-N`" + ` // Pan with offset (0-50), e.g., ` + "`right-25`" + `

			*Examples:*
			1. !sticker https://demo.alyza.site nocrop start=00:00 end=00:02 fps=24 quality=80
			2. !sticker https://demo.alyza.site/ direction=left-30 quality=90
		`)

	case "USER":
		message = strings.TrimSpace(`
			*LIST COMMANDS*
			1. ` + "`!token`" + `
		`)
	case "ADMIN":
		message = strings.TrimSpace(`
			*LIST COMMANDS*
			1. ` + "`!listmapel`" + `
			2. ` + "`!pdf <nomor dari !listmapel>`" + `
			3. ` + "`!pdf <nama mapel>`" + `
			4. ` + "`!answer <nomor dari !listmapel <jawaban>`" + `
			5. ` + "`!answer <nama mapel> <jawaban>`" + `
		`)
	case "OWNER":
		message = strings.TrimSpace(`
			*LIST COMMANDS*

			*ADMIN*
			1. ` + "`!listmapel`" + `
			2. ` + "`!pdf <nomor dari !listmapel>`" + `
			3. ` + "`!pdf <nama mapel>`" + `
			4. ` + "`!answer <nomor dari !listmapel <jawaban>`" + `
			5. ` + "`!answer <nama mapel> <jawaban>`" + `

			*USER*
			1. ` + "`!token`" + `

			*COMMON*
			_From URL:_
			- ` + "`!sticker <video/gif/image URL>`" + `

			_Send with image/video/gif:_
			- ` + "`!sticker`" + `

			_Optional parameters_ (can be added after the command or URL):
			- ` + "`nocrop`" + ` // Prevent auto-cropping to square
			- ` + "`start=MM:SS`" + ` // Start time for video/gif
			- ` + "`end=MM:SS`" + ` // End time for video/gif
			- ` + "`fps=N`" + ` // Frame per second (1-60)
			- ` + "`quality=N`" + ` // Output quality (1-100)
			- ` + "`direction=side`" + ` // Pan direction: up, down, left, right
			- ` + "`direction=side-N`" + ` // Pan with offset (0-50), e.g., ` + "`right-25`" + `

			*Examples:*
			1. !sticker https://demo.alyza.site nocrop start=00:00 end=00:02 fps=24 quality=80
			2. !sticker https://demo.alyza.site/ direction=left-30 quality=90
		`)
	}

	lines := strings.Split(message, "\n")
	for i := range lines {
		lines[i] = strings.TrimLeft(lines[i], "\t ")
	}
	message = strings.Join(lines, "\n")

	fmt.Printf("DEBUG: Sending help message to %s\n", senderJID.String())
	if err := uc.waClient.SendMessageToJID(ctxTyped, senderJID, message); err != nil {
		fmt.Printf("DEBUG: Error sending message: %v\n", err)
	} else {
		fmt.Printf("DEBUG: Message sent successfully to %s\n", senderJID.String())
	}
}

func (uc *HandlerUseCase) HandleCancel(senderJID string) {
	state := uc.StateRepo.GetUserStateSimple(senderJID)
	if state == "" {
		uc.waClient.SendMessage(context.Background(), senderJID, "❌ There is no running process")
		return
	}

	err := uc.StateRepo.CancelUserState(senderJID)
	if err != nil {
		uc.waClient.SendMessage(context.Background(), senderJID, "⚠️ Failed to cancel process")
		return
	}

	uc.waClient.SendMessage(context.Background(), senderJID, "✅ Process successfully cancelled")
}

func (uc *HandlerUseCase) HandlePendingToken(ctx interface{}, senderJID waTypes.JID, messageText string) {
	ctxTyped := ctx.(context.Context)
	uc.TokenUC.HandleNameInput(ctxTyped, senderJID, messageText)
}
