package config

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/joho/godotenv"
	"github.com/mdp/qrterminal"
	waLog "go.mau.fi/whatsmeow/util/log"

	"wa-bot/internal/delivery/cron"
	"wa-bot/internal/delivery/http"
	"wa-bot/internal/delivery/whatsapp"
	"wa-bot/internal/infrastructure/ai"
	"wa-bot/internal/infrastructure/api"
	infrastructureConfig "wa-bot/internal/infrastructure/config"
	"wa-bot/internal/infrastructure/media"
	"wa-bot/internal/infrastructure/storage"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
	"wa-bot/internal/usecase"
)

type App struct {
	waClient      *whatsappInfra.WhatsAppClient
	eventHandler  *whatsapp.WhatsAppEventHandler
	httpServer    *http.HTTPServer
	cronScheduler *cron.CronScheduler
	config        *infrastructureConfig.EnvConfig
}

func (a *App) Run() error {
	a.waClient.AddEventHandler(func(evt interface{}) {
		a.eventHandler.HandleEvent(evt)
	})

	if err := a.cronScheduler.Start(); err != nil {
		return fmt.Errorf("failed to start cron scheduler: %w", err)
	}

	if a.waClient.GetClient() == nil {
		return fmt.Errorf("whatsapp client not initialized")
	}

	if !a.waClient.IsLoggedIn() {
		qrChan, err := a.waClient.GetQRChannel(context.Background())
		if err != nil {
			return fmt.Errorf("failed to get QR channel: %w", err)
		}

		err = a.waClient.Connect()
		if err != nil {
			return fmt.Errorf("failed to connect to WhatsApp: %w", err)
		}

		fmt.Println("Waiting for QR code scan...")
		for evt := range qrChan {
			if evt.Event == "code" {
				fmt.Println("\n=== SCAN QR CODE BELOW ===")
				qrterminal.GenerateHalfBlock(evt.Code, qrterminal.L, os.Stdout)
				fmt.Println("\n=== OR SCAN THIS STRING ===")
				fmt.Println(evt.Code)
				fmt.Println("===========================\n")
			} else if evt.Event == "success" {
				fmt.Println("\n✅ Successfully authenticated!")
				break
			} else {
				fmt.Println("Login event:", evt.Event)
			}
		}
	} else {
		err := a.waClient.Connect()
		if err != nil {
			return fmt.Errorf("failed to connect to WhatsApp: %w", err)
		}
		fmt.Println("✅ Successfully authenticated!")
	}

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	<-c

	fmt.Println("\nShutting down...")
	a.waClient.Disconnect()
	a.cronScheduler.Stop()

	return nil
}

func InitializeApp() (*App, error) {
	if err := godotenv.Load(); err != nil {
		return nil, fmt.Errorf("failed to load .env file: %w", err)
	}

	cfg := infrastructureConfig.NewEnvConfig()
	stateRepo := storage.NewInMemoryUserState()

	logLevel := cfg.Get("LOG_LEVEL")
	if logLevel == "" {
		logLevel = "DEBUG"
	}
	dbURL := "file:wa-bot-session.db?_foreign_keys=on"
	dbLog := waLog.Stdout("Database", "WARN", true)

	waClient, err := whatsappInfra.NewWhatsAppClient(dbURL, logLevel, dbLog)
	if err != nil {
		return nil, fmt.Errorf("failed to create WhatsApp client: %w", err)
	}

	apiKey := cfg.Get("GEMINI_API_KEY")
	geminiService, err := ai.NewGeminiService(apiKey)
	if err != nil {
		return nil, fmt.Errorf("failed to create Gemini service: %w", err)
	}

	mediaDownloader := media.NewMediaDownloader()

	pdfClient := api.NewPDFClient(cfg.Get("PDF_URL"))
	tokenClient := api.NewTokenClient(cfg.Get("API_URL"))
	apiRepository := api.NewAPIRepository(pdfClient, tokenClient)

	waService := usecase.NewWhatsAppService(waClient, cfg)

	stickerUC := usecase.NewStickerUseCase(waClient, mediaDownloader, stateRepo, cfg)
	pdfUC := usecase.NewPDFUseCase(waClient, apiRepository, geminiService, stateRepo)
	tokenUC := usecase.NewTokenUseCase(waClient, apiRepository, stateRepo, cfg)
	adminUC := usecase.NewAdminUseCase(waClient, apiRepository, cfg)
	handlerUC := usecase.NewHandlerUseCase(stickerUC, pdfUC, tokenUC, adminUC, waService, stateRepo, waClient)

	deliveryWaService := whatsapp.NewWhatsAppService(waClient, cfg)
	eventHandler := whatsapp.NewWhatsAppEventHandler(handlerUC, deliveryWaService, stateRepo, waClient)
	commandRouter := whatsapp.NewCommandRouter(handlerUC)
	_ = commandRouter

	httpServer := http.NewHTTPServer(waClient, cfg)
	cronScheduler := cron.NewCronScheduler(waClient, dbURL, cfg.Get("CRON_SCHEDULE"))

	app := &App{
		waClient:      waClient,
		eventHandler:  eventHandler,
		httpServer:    httpServer,
		cronScheduler: cronScheduler,
		config:        cfg,
	}

	return app, nil
}
