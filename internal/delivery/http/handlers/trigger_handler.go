package handlers

import (
	"encoding/json"
	"net/http"

	"wa-bot/internal/delivery/http/dto"
	"wa-bot/internal/domain/entity"
)

type TriggerHandler struct {
	handler *Handler
}

func NewTriggerHandler(h *Handler) *TriggerHandler {
	return &TriggerHandler{handler: h}
}

func (th *TriggerHandler) GetTriggers(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if th.handler.triggerRepo == nil {
		th.handler.sendError(w, http.StatusInternalServerError, "Repository not configured")
		return
	}

	triggers, err := th.handler.triggerRepo.GetAll(r.Context())
	if err != nil {
		th.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	if triggers == nil {
		triggers = []*entity.Trigger{}
	}

	th.handler.sendJSON(w, triggers)
}

func (th *TriggerHandler) CreateTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	var t entity.Trigger
	if err := json.NewDecoder(r.Body).Decode(&t); err != nil {
		th.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	if t.ID == "" {
		t.ID = generateID()
	}

	if err := th.handler.triggerRepo.Create(r.Context(), &t); err != nil {
		th.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	w.WriteHeader(http.StatusCreated)
	th.handler.sendJSON(w, t)
}

func (th *TriggerHandler) UpdateTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	id := th.handler.getJID(r, "id")

	var t entity.Trigger
	if err := json.NewDecoder(r.Body).Decode(&t); err != nil {
		th.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	t.ID = id
	if err := th.handler.triggerRepo.Update(r.Context(), &t); err != nil {
		th.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	th.handler.sendJSON(w, t)
}

func (th *TriggerHandler) DeleteTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	id := th.handler.getJID(r, "id")

	if err := th.handler.triggerRepo.Delete(r.Context(), id); err != nil {
		th.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	w.WriteHeader(http.StatusOK)
	th.handler.sendSuccess(w, nil)
}

func (th *TriggerHandler) DeleteAllTriggers(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if err := th.handler.triggerRepo.DeleteAll(r.Context()); err != nil {
		th.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	w.WriteHeader(http.StatusOK)
	th.handler.sendSuccess(w, nil)
}

func (th *TriggerHandler) TestTrigger(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if th.handler.lua == nil {
		th.handler.sendError(w, http.StatusInternalServerError, "Lua service not initialized")
		return
	}

	var req dto.TestTriggerRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		th.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	result, err := th.handler.lua.TestTrigger(r.Context(), req.Pattern, req.Script, req.Message)
	if err != nil {
		th.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	th.handler.sendJSON(w, result)
}
