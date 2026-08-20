package handlers

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/gorilla/mux"

	"wa-bot/internal/delivery/http/dto"
)

type MessageHandler struct {
	handler *Handler
}

func NewMessageHandler(h *Handler) *MessageHandler {
	return &MessageHandler{handler: h}
}

func (mh *MessageHandler) SendMessage(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req dto.SendMessageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	if !mh.validateSecret(req.Secret) {
		mh.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	fmt.Printf("[SEND] Sending message to: %s | Content: %s\n", req.Target, req.Message)

	id, err := mh.handler.client.SendMessage(context.Background(), req.Target, req.Message, false)
	if err != nil {
		fmt.Printf("[ERR] Failed to send WhatsApp message: %v\n", err)
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	fmt.Printf("[OK] WhatsApp message sent to %s\n", req.Target)

	mh.handler.sendJSONWithStatus(w, http.StatusOK, map[string]interface{}{
		"status": "success",
		"id":     id,
	})
}

func (mh *MessageHandler) SendMedia(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if err := r.ParseMultipartForm(50 << 20); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, "Failed to parse form: "+err.Error())
		return
	}

	secret := r.FormValue("secret")
	target := r.FormValue("target")
	message := r.FormValue("message")
	mediaType := strings.ToLower(strings.TrimSpace(r.FormValue("type")))

	if !mh.validateSecret(secret) {
		mh.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	file, header, err := r.FormFile("file")
	if err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, "Failed to get file: "+err.Error())
		return
	}
	defer file.Close()

	data, err := io.ReadAll(file)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, "Failed to read file: "+err.Error())
		return
	}
	if len(data) == 0 {
		mh.handler.sendError(w, http.StatusBadRequest, "Empty file")
		return
	}

	fmt.Printf("[SEND] Sending %s to: %s (%d bytes)\n", mediaType, target, len(data))

	os.MkdirAll("media", 0755)

	isAudio := mediaType == "audio" || mediaType == "ptt" || mediaType == "voice" || mediaType == "audio-ptt"

	// Parse PTT early so mimetype / extension can be normalised correctly.
	// PTT (voice note) MUST be `audio/ogg; codecs=opus` for WhatsApp to render
	// the bubble player; any other codec shows as a generic audio file.
	pttEarly := mediaType == "ptt" || mediaType == "audio-ptt"
	if pttStr := r.FormValue("ptt"); pttStr != "" {
		if b, err := strconv.ParseBool(pttStr); err == nil {
			pttEarly = b
		}
	}

	// Detect mimetype for audio so we can pick a sensible extension and
	// pass a real audio mimetype to WhatsApp.
	var audioMimetype string
	if isAudio {
		if pttEarly {
			// PTT must be opus-in-ogg for the bubble player.
			// If source is already ogg/opus/webm, keep bytes as-is.
			// If source is mp3/m4a/wav (e.g. /home/aorus/tts-generate mp3s),
			// transcode with ffmpeg (already in Dockerfile) to opus.
			detected := http.DetectContentType(data)
			if strings.Contains(detected, "ogg") || strings.Contains(detected, "opus") ||
				strings.HasSuffix(strings.ToLower(header.Filename), ".ogg") ||
				strings.HasSuffix(strings.ToLower(header.Filename), ".opus") {
				audioMimetype = "audio/ogg; codecs=opus"
			} else {
				opusData, err := transcodeToOpusOgg(data)
				if err != nil {
					fmt.Printf("[WARN] PTT transcode failed (%v), sending as-is with opus mimetype\n", err)
					audioMimetype = "audio/ogg; codecs=opus"
				} else {
					data = opusData
					audioMimetype = "audio/ogg; codecs=opus"
					fmt.Printf("[SEND] Transcoded PTT to opus %d -> %d bytes\n", len(data), len(opusData))
				}
			}
		} else {
			audioMimetype = http.DetectContentType(data)
			if !strings.HasPrefix(audioMimetype, "audio/") {
				// Fallback via filename extension before defaulting to opus.
				switch strings.ToLower(filepath.Ext(header.Filename)) {
				case ".mp3":
					audioMimetype = "audio/mpeg"
				case ".m4a":
					audioMimetype = "audio/mp4"
				case ".wav":
					audioMimetype = "audio/wav"
				case ".webm":
					audioMimetype = "audio/webm"
				case ".opus":
					audioMimetype = "audio/ogg; codecs=opus"
				default:
					audioMimetype = "audio/ogg"
				}
			}
		}
	}

	ext := filepath.Ext(header.Filename)
	// PTT is always OGG Opus after transcode — force .ogg even if input was .mp3
	// so the saved file matches the transcoded bytes.
	if isAudio && strings.Contains(audioMimetype, "opus") {
		ext = ".ogg"
	} else if ext == "" {
		switch {
		case mediaType == "image":
			ext = ".jpg"
		case mediaType == "video":
			ext = ".mp4"
		case isAudio:
			switch {
			case strings.HasPrefix(audioMimetype, "audio/ogg"):
				ext = ".ogg"
			case audioMimetype == "audio/mpeg":
				ext = ".mp3"
			case audioMimetype == "audio/mp4":
				ext = ".m4a"
			case audioMimetype == "audio/opus":
				ext = ".opus"
			case audioMimetype == "audio/wav":
				ext = ".wav"
			case audioMimetype == "audio/webm":
				ext = ".webm"
			default:
				ext = ".ogg"
			}
		default:
			ext = ".bin"
		}
	}
	safeJID := strings.ReplaceAll(target, "@", "_")
	safeJID = strings.ReplaceAll(safeJID, ".", "_")
	filename := fmt.Sprintf("sent_%d_%s%s", time.Now().UnixMilli(), safeJID, ext)
	mediaURL := fmt.Sprintf("/media/%s", filename)

	if _, err := mh.handler.storage.Save(r.Context(), filename, bytes.NewReader(data)); err != nil {
		fmt.Printf("Failed to persist sent media: %v\n", err)
		mediaURL = ""
	}

	ctx := context.Background()
	var id string
	var sendErr error

	switch mediaType {
	case "image":
		id, sendErr = mh.handler.client.SendImage(ctx, target, data, message, mediaURL, false)
	case "video":
		id, sendErr = mh.handler.client.SendVideo(ctx, target, data, message, mediaURL, false)
	case "audio", "ptt", "voice", "audio-ptt":
		ptt := pttEarly
		var seconds uint32
		if secStr := r.FormValue("seconds"); secStr != "" {
			if s, err := strconv.ParseUint(secStr, 10, 32); err == nil {
				seconds = uint32(s)
			}
		}
		var waveform []byte
		if wfStr := r.FormValue("waveform"); wfStr != "" {
			if wf, err := base64.StdEncoding.DecodeString(wfStr); err == nil {
				waveform = wf
			}
		}
		fmt.Printf("[SEND] audio mimetype=%s ptt=%v seconds=%d bytes=%d media=%s\n", audioMimetype, ptt, seconds, len(data), mediaURL)
		id, sendErr = mh.handler.client.SendAudio(ctx, target, data, audioMimetype, ptt, seconds, waveform, mediaURL)
	default:
		id, sendErr = mh.handler.client.SendDocument(ctx, target, data, header.Filename, mediaURL, false)
	}

	if sendErr != nil {
		fmt.Printf("[ERR] Failed to send media: %v\n", sendErr)
		mh.handler.sendError(w, http.StatusInternalServerError, sendErr.Error())
		return
	}

	fmt.Printf("[OK] Media sent successfully to %s\n", target)

	mh.handler.sendJSONWithStatus(w, http.StatusOK, map[string]interface{}{
		"status": "success",
		"id":     id,
	})
}

func (mh *MessageHandler) SendSticker(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req dto.SendStickerRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	if !mh.validateSecret(req.Secret) {
		mh.handler.sendError(w, http.StatusUnauthorized, "Unauthorized")
		return
	}

	localPath := strings.TrimPrefix(req.MediaURL, "/")
	localPath = strings.TrimPrefix(localPath, "api/")

	fmt.Printf("[SEND] Sending sticker to: %s | Path: %s\n", req.Target, localPath)

	reader, err := mh.handler.storage.Get(context.Background(), localPath)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, "Failed to read sticker file: "+err.Error())
		return
	}
	data, err := io.ReadAll(reader)
	reader.Close()
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, "Failed to read sticker file: "+err.Error())
		return
	}

	id, err := mh.handler.client.SendSticker(context.Background(), req.Target, data, req.IsAnimated, req.MediaURL, false)
	if err != nil {
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	mh.handler.sendJSONWithStatus(w, http.StatusOK, map[string]interface{}{
		"status": "success",
		"id":     id,
	})
}

func (mh *MessageHandler) BulkSendSame(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req dto.BulkSendSameRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(err.Error()))
		return
	}

	if !mh.validateSecret(req.Secret) {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte("Unauthorized"))
		return
	}

	done := make(chan bool)
	for _, target := range req.Targets {
		go func(targetJID string) {
			mh.handler.client.SendMessage(context.Background(), targetJID, req.Message, false)
			done <- true
		}(target)
	}

	for range req.Targets {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk same message sent successfully"))
}

func (mh *MessageHandler) BulkSendDifferent(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var req dto.BulkSendDifferentRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(err.Error()))
		return
	}

	if !mh.validateSecret(req.Secret) {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte("Unauthorized"))
		return
	}

	done := make(chan bool)
	for _, msg := range req.Messages {
		go func(targetJID, message string) {
			mh.handler.client.SendMessage(context.Background(), targetJID, message, false)
			done <- true
		}(msg.Targets, msg.Message)
	}

	for range req.Messages {
		<-done
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Bulk different messages sent successfully"))
}

func (mh *MessageHandler) validateSecret(secret string) bool {
	SECRET := os.Getenv("API_SECRET")
	if SECRET == "" {
		SECRET = "default-secret"
	}
	return secret == SECRET
}

// transcodeToOpusOgg converts arbitrary audio bytes to OGG/Opus via ffmpeg.
// Requires ffmpeg in PATH (/usr/local/bin/ffmpeg in Docker, /usr/bin/ffmpeg locally).
func transcodeToOpusOgg(in []byte) ([]byte, error) {
	inFile, err := os.CreateTemp("", "wa-audio-in-*")
	if err != nil {
		return nil, err
	}
	defer os.Remove(inFile.Name())
	if _, err := inFile.Write(in); err != nil {
		inFile.Close()
		return nil, err
	}
	inFile.Close()

	outFile, err := os.CreateTemp("", "wa-audio-out-*.ogg")
	if err != nil {
		return nil, err
	}
	outPath := outFile.Name()
	outFile.Close()
	defer os.Remove(outPath)

	// Opus in OGG, mono-friendly, low bitrate matching WhatsApp PTT.
	cmd := exec.Command("ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", inFile.Name(), "-c:a", "libopus", "-b:a", "32k", "-vbr", "on", "-compression_level", "10", outPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		return nil, fmt.Errorf("ffmpeg: %v: %s", err, string(out))
	}
	return os.ReadFile(outPath)
}

func (mh *MessageHandler) SendReaction(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["chatId"]
	msgID := vars["id"]

	var req dto.ReactMessageRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	fmt.Printf("[REACT] Reacting %s to msg %s in chat %s\n", req.Emoji, msgID, chatID)
	if err := mh.handler.client.SendReaction(chatID, msgID, req.Emoji); err != nil {
		fmt.Printf("[ERR] Failed to send reaction: %v\n", err)
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	mh.handler.sendJSONWithStatus(w, http.StatusOK, map[string]interface{}{
		"status": "success",
	})
}

func (mh *MessageHandler) SendTyping(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	vars := mux.Vars(r)
	chatID := vars["chatId"]

	var req dto.TypingRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		mh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	err := mh.handler.client.SendPresence(chatID, req.IsTyping)
	if err != nil {
		fmt.Printf("[ERR] Failed to send typing: %v\n", err)
		mh.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	mh.handler.sendJSONWithStatus(w, http.StatusOK, map[string]interface{}{
		"status": "success",
	})
}
