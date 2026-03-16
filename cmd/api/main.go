package main

import (
	"context"
	"fmt"
	"os"

	"github.com/joho/godotenv"
	"github.com/mdp/qrterminal"
	waLog "go.mau.fi/whatsmeow/util/log"

	"wa-bot/internal/delivery/cron"
	"wa-bot/internal/delivery/http"
	"wa-bot/internal/delivery/whatsapp"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/ai"
	infrastructureConfig "wa-bot/internal/infrastructure/config"
	"wa-bot/internal/infrastructure/lua"
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

	// Start session management in background
	go a.HandleSession()

	// Start HTTP server (this is blocking)
	port := a.config.Get("PORT")
	if port == "" {
		port = "3000"
	}
	fmt.Printf("Starting HTTP server on port %s...\n", port)
	if err := a.httpServer.Start(); err != nil {
		return fmt.Errorf("failed to start HTTP server: %w", err)
	}

	return nil
}

func (a *App) HandleSession() {
	if !a.waClient.IsLoggedIn() {
		qrChan, err := a.waClient.GetQRChannel(context.Background())
		if err != nil {
			fmt.Printf("failed to get QR channel: %v\n", err)
			return
		}

		err = a.waClient.Connect()
		if err != nil {
			fmt.Printf("failed to connect to WhatsApp: %v\n", err)
			return
		}

		fmt.Println("Waiting for QR code scan (check frontend or terminal)...")
		for evt := range qrChan {
			if evt.Event == "code" {
				fmt.Println("\n=== SCAN QR CODE IN FRONTEND ===")
				qrterminal.GenerateHalfBlock(evt.Code, qrterminal.L, os.Stdout)

				a.httpServer.BroadcastMessage("qr_code", map[string]string{
					"code": evt.Code,
				})
			} else if evt.Event == "success" {
				fmt.Println("\n[AUTH] Successfully authenticated!")
				a.httpServer.BroadcastMessage("auth_success", nil)
				break
			} else {
				fmt.Println("Login event:", evt.Event)
			}
		}
	} else {
		err := a.waClient.Connect()
		if err != nil {
			fmt.Printf("failed to connect to WhatsApp: %v\n", err)
			return
		}
		fmt.Println("[AUTH] Successfully authenticated!")
		a.httpServer.BroadcastMessage("auth_success", nil)
	}
}

func InitializeApp() (*App, error) {
	if err := godotenv.Load(); err != nil {
		return nil, fmt.Errorf("failed to load .env file: %w", err)
	}

	cfg := infrastructureConfig.NewEnvConfig()
	storageRepo := storage.NewLocalStorage("media")
	stateRepo := storage.NewInMemoryUserState()

	logLevel := cfg.Get("LOG_LEVEL")
	if logLevel == "" {
		logLevel = "INFO"
	}
	dbURL := "file:database/wa-bot-session.db?_foreign_keys=on"
	dbLog := waLog.Stdout("Database", "WARN", true)

	waClient, err := whatsappInfra.NewWhatsAppClient(dbURL, logLevel, dbLog)
	if err != nil {
		return nil, fmt.Errorf("failed to create WhatsApp client: %w", err)
	}

	apiKey := cfg.Get("GEMINI_API_KEY")
	geminiService, err := ai.NewGeminiService(apiKey, storageRepo)
	if err != nil {
		return nil, fmt.Errorf("failed to create Gemini service: %w", err)
	}

	mediaDownloader := media.NewMediaDownloader(storageRepo)
	redisService := storage.NewRedisService(cfg)

	handlerUC := usecase.NewHandlerUseCase(stateRepo, waClient)
	deliveryWaService := whatsapp.NewWhatsAppService(waClient, cfg)
	eventHandler := whatsapp.NewWhatsAppEventHandler(handlerUC, deliveryWaService, stateRepo, waClient, storageRepo)

	httpServer := http.NewHTTPServer(waClient, cfg, storageRepo)

	msgStore, err := repository.NewMessageStore("file:database/wa-bot-messages.db?_foreign_keys=on")
	if err != nil {
		return nil, fmt.Errorf("failed to create message store: %w", err)
	}
	httpServer.SetMessageRepo(msgStore)

	appStore, err := repository.NewAppStore("file:database/wa-bot-app.db?_foreign_keys=on")
	if err != nil {
		return nil, fmt.Errorf("failed to create app store: %w", err)
	}

	luaService := lua.NewLuaService(waClient, appStore, stateRepo, storageRepo, geminiService, mediaDownloader, redisService)
	eventHandler.SetLuaService(luaService)
	httpServer.SetLuaService(luaService)
	httpServer.SetTriggerRepo(appStore)
	httpServer.SetCronRepo(appStore)

	cronScheduler := cron.NewCronScheduler(appStore, luaService)
	httpServer.SetCronScheduler(cronScheduler)

	waClient.SetLogger(httpServer)
	eventHandler.SetMessageStore(msgStore)
	eventHandler.SetHTTPServer(httpServer)

	return &App{
		waClient:      waClient,
		eventHandler:  eventHandler,
		httpServer:    httpServer,
		cronScheduler: cronScheduler,
		config:        cfg,
	}, nil
}

func main() {
	app, err := InitializeApp()
	if err != nil {
		panic(err)
	}

	if err := app.Run(); err != nil {
		panic(err)
	}

	fmt.Println("Application stopped gracefully")
}
