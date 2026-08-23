package main

import (
	"context"
	"fmt"
	"os"
	"time"

	"github.com/joho/godotenv"
	"github.com/mdp/qrterminal"
	waLog "go.mau.fi/whatsmeow/util/log"

	"wa-bot/internal/delivery/cron"
	"wa-bot/internal/delivery/http"
	"wa-bot/internal/delivery/whatsapp"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/ai"
	"wa-bot/internal/infrastructure/call"
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
	callSvc       *call.CallService
	config        *infrastructureConfig.EnvConfig
}

func (a *App) Run() error {
	// Bind port early so Electron can detect BACKEND_PORT immediately
	ln, actualPort, err := a.httpServer.BindPort()
	if err != nil {
		return fmt.Errorf("failed to bind port: %w", err)
	}
	fmt.Printf("BACKEND_PORT:%d\n", actualPort)
	fmt.Printf("Server listening on port %d, initializing WhatsApp...\n", actualPort)

	a.waClient.AddEventHandler(func(evt interface{}) {
		a.eventHandler.HandleEvent(evt)
	})

	if err := a.cronScheduler.Start(); err != nil {
		ln.Close()
		return fmt.Errorf("failed to start cron scheduler: %w", err)
	}

	if a.waClient.GetClient() == nil {
		ln.Close()
		return fmt.Errorf("whatsapp client not initialized")
	}

	// Start session management in background
	go a.HandleSession()

	// Start HTTP server using the pre-bound listener
	if err := a.httpServer.StartWithListener(ln); err != nil {
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
	// Load .env if available; optional for desktop mode
	_ = godotenv.Load()

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
	historySyncService := whatsapp.NewHistorySyncService(waClient, msgStore)
	historySyncService.SetBroadcaster(httpServer)
	httpServer.SetHistorySync(historySyncService)
	eventHandler.SetHistorySyncService(historySyncService)

	appStore, err := repository.NewAppStore("file:database/wa-bot-app.db?_foreign_keys=on")
	if err != nil {
		return nil, fmt.Errorf("failed to create app store: %w", err)
	}

	// TTS + AI settings are DB-backed (env ya env, settings ya settings). On
	// first boot with an empty app_settings table, migrate the legacy env vars
	// into the DB so existing deployments keep working. After this one-time
	// migration the app reads these settings exclusively from the DB.
	settingsRepo := repository.NewAppSettingsRepository(appStore)
	seedCtx, seedCancel := context.WithTimeout(context.Background(), 10*time.Second)
	if err := repository.SeedSettings(seedCtx, settingsRepo, map[string]string{
		"gemini_api_key":               os.Getenv("GEMINI_API_KEY"),
		"ai_server_url":                os.Getenv("AI_SERVER_URL"),
		"call_tts_provider":            os.Getenv("CALL_TTS_PROVIDER"),
		"call_tts_default_voice":       os.Getenv("CALL_TTS_DEFAULT_VOICE"),
		"call_tts_fish_audio_key":      os.Getenv("CALL_TTS_FISH_AUDIO_KEY"),
		"call_tts_fish_audio_model":    os.Getenv("CALL_TTS_FISH_AUDIO_MODEL"),
		"call_tts_fish_audio_voice_id": os.Getenv("CALL_TTS_FISH_AUDIO_VOICE_ID"),
	}); err != nil {
		seedCancel()
		return nil, fmt.Errorf("failed to seed settings: %w", err)
	}
	seedCancel()
	httpServer.SetSettingsRepo(settingsRepo)

	// Gemini service, gated by the DB-backed gemini_api_key setting.
	var geminiService *ai.GeminiService
	geminiAPIKey, _ := settingsRepo.Get(context.Background(), "gemini_api_key")
	if geminiAPIKey != "" {
		geminiService, err = ai.NewGeminiService(geminiAPIKey, storageRepo)
		if err != nil {
			fmt.Printf("Warning: Failed to create Gemini service: %v\n", err)
		}
	}
	httpServer.SetGeminiService(geminiService)

	callSvc := call.NewCallService(waClient.GetCallClient(), waClient.IsConnected, appStore, httpServer)
	httpServer.SetCallService(callSvc)

	// Wire the API key repository (also the external-call auth store).
	httpServer.SetAPIKeyRepo(appStore)

	// Ring timeout (PRD §29), default 45s.
	if ringSeconds := cfg.GetInt("CALL_RING_TIMEOUT_SECONDS"); ringSeconds > 0 {
		callSvc.SetRingTimeout(time.Duration(ringSeconds) * time.Second)
	}

	// TTS provider, gated by the DB-backed call_tts_provider setting
	// (edge | fish). Unknown/empty values leave the provider nil (tts_unavailable).
	var ttsProvider call.TTSProvider
	switch ttsProviderSetting, _ := settingsRepo.Get(context.Background(), "call_tts_provider"); ttsProviderSetting {
	case "edge":
		voice, _ := settingsRepo.Get(context.Background(), "call_tts_default_voice")
		ttsProvider = call.NewEdgeTTSProvider(voice)
	case "fish":
		fishKey, _ := settingsRepo.Get(context.Background(), "call_tts_fish_audio_key")
		fishModel, _ := settingsRepo.Get(context.Background(), "call_tts_fish_audio_model")
		fishVoiceID, _ := settingsRepo.Get(context.Background(), "call_tts_fish_audio_voice_id")
		ttsProvider = call.NewFishAudioTTSProvider(call.FishAudioConfig{
			APIKey:         fishKey,
			DefaultModel:   fishModel,
			DefaultVoiceID: fishVoiceID,
		})
	}
	httpServer.SetTTSProvider(ttsProvider)

	callMediaHandler := http.NewCallMediaHandler(callSvc)
	httpServer.SetCallMediaHandler(callMediaHandler.ServeWS)
	if err := callSvc.MarkInterruptedOnStartup(context.Background()); err != nil {
		fmt.Printf("Warning: failed to mark interrupted calls: %v\n", err)
	}

	luaService := lua.NewLuaService(waClient, appStore, stateRepo, storageRepo, geminiService, mediaDownloader, redisService)
	eventHandler.SetLuaService(luaService)
	httpServer.SetLuaService(luaService)
	httpServer.SetTriggerRepo(appStore)
	httpServer.SetCronRepo(appStore)
	httpServer.SetWebhookRepo(appStore)
	httpServer.SetWebhookLogRepo(appStore)

	cronScheduler := cron.NewCronScheduler(appStore, luaService)
	httpServer.SetCronScheduler(cronScheduler)

	waClient.SetLogger(httpServer)
	eventHandler.SetMessageStore(msgStore)
	eventHandler.SetHTTPServer(httpServer)

	// Wire up AI companion client (disabled if ai_server_url setting is empty)
	aiServerURL, _ := settingsRepo.Get(context.Background(), "ai_server_url")
	aiClient := ai.NewAIClient(aiServerURL)
	eventHandler.SetAIClient(aiClient)

	return &App{
		waClient:      waClient,
		eventHandler:  eventHandler,
		httpServer:    httpServer,
		cronScheduler: cronScheduler,
		callSvc:       callSvc,
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
