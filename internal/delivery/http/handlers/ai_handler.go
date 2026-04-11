package handlers

import (
	"context"
	_ "embed"
	"fmt"
	"net/http"
)

//go:embed docs/api-reference.md
var apiDocs []byte

type AIHandler struct {
	handler *Handler
}

func NewAIHandler(h *Handler) *AIHandler {
	return &AIHandler{handler: h}
}

func (ah *AIHandler) GetDocs(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/markdown")
	w.Write(apiDocs)
}

func (ah *AIHandler) ChatAssistant(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Prompt      string `json:"prompt"`
		CurrentCode string `json:"currentCode,omitempty"`
		Model       string `json:"model,omitempty"`
	}

	if err := ah.handler.readJSON(r, &req); err != nil {
		ah.handler.sendError(w, http.StatusBadRequest, "Invalid request body")
		return
	}

	gemini := ah.handler.GetGeminiService()
	if gemini == nil {
		ah.handler.sendError(w, http.StatusServiceUnavailable, "AI Service not available")
		return
	}

	systemPrompt := fmt.Sprintf(`You are an AI Coding Agent for a WhatsApp bot platform. 
Your goal is to help users write, REFACTOR, and FIX Lua scripts.
Use the following documentation as your primary knowledge source:

---
%s
---

AGENT GUIDELINES:
1. When the user asks for a change, provide the FULL updated Lua script in a single code block.
2. Ensure the code is complete, correct, and follows the documentation.
3. If you are refactoring "Current Code", maintain the existing logic unless asked otherwise.
4. Be concise in your explanations.
5. Focus on providing high-quality, production-ready Lua code.`, string(apiDocs))

	var fullPrompt string
	if req.CurrentCode != "" {
		fullPrompt = fmt.Sprintf("%s\n\nCONTEXT - CURRENT CODE:\n```lua\n%s\n```\n\nUSER REQUEST: %s\n\nProvide the updated code and explain your changes.", systemPrompt, req.CurrentCode, req.Prompt)
	} else {
		fullPrompt = fmt.Sprintf("%s\n\nUser Question: %s", systemPrompt, req.Prompt)
	}

	model := req.Model
	if model == "" {
		model = "gemma-3-27b-it"
	}

	answer, err := gemini.GenerateText(context.Background(), model, fullPrompt)
	if err != nil {
		ah.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	ah.handler.sendJSON(w, map[string]string{
		"answer": answer,
	})
}
