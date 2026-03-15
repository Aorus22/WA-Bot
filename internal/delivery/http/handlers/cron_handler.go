package handlers

import (
	"net/http"
	"wa-bot/internal/delivery/cron"
	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/lua"

	"github.com/google/uuid"
	"github.com/gorilla/mux"
)

type CronHandler struct {
	handler    *Handler
	repo       repository.CronJobRepository
	scheduler  *cron.CronScheduler
	luaService *lua.LuaService
}

func NewCronHandler(repo repository.CronJobRepository, scheduler *cron.CronScheduler, luaService *lua.LuaService) *CronHandler {
	return &CronHandler{
		repo:       repo,
		scheduler:  scheduler,
		luaService: luaService,
	}
}

func (h *CronHandler) SetHandler(handler *Handler) {
	h.handler = handler
}

func (h *CronHandler) GetAll(w http.ResponseWriter, r *http.Request) {
	jobs, err := h.repo.GetAllCron(r.Context())
	if err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}
	h.handler.sendJSON(w, jobs)
}

func (h *CronHandler) Create(w http.ResponseWriter, r *http.Request) {
	var job entity.CronJob
	if err := h.handler.readJSON(r, &job); err != nil {
		h.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	job.ID = uuid.New().String()
	if err := h.repo.CreateCron(r.Context(), &job); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	_ = h.scheduler.Reload()

	h.handler.sendJSONWithStatus(w, http.StatusCreated, job)
}

func (h *CronHandler) Update(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	id := vars["id"]

	var job entity.CronJob
	if err := h.handler.readJSON(r, &job); err != nil {
		h.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	job.ID = id
	if err := h.repo.UpdateCron(r.Context(), &job); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	_ = h.scheduler.Reload()

	h.handler.sendJSON(w, job)
}

func (h *CronHandler) Delete(w http.ResponseWriter, r *http.Request) {
	vars := mux.Vars(r)
	id := vars["id"]

	if err := h.repo.DeleteCron(r.Context(), id); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	_ = h.scheduler.Reload()

	w.WriteHeader(http.StatusNoContent)
}

func (h *CronHandler) DeleteAll(w http.ResponseWriter, r *http.Request) {
	if err := h.repo.DeleteAllCron(r.Context()); err != nil {
		h.handler.sendError(w, http.StatusInternalServerError, err.Error())
		return
	}

	_ = h.scheduler.Reload()

	w.WriteHeader(http.StatusNoContent)
}

func (h *CronHandler) Test(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Script string `json:"script"`
	}
	if err := h.handler.readJSON(r, &req); err != nil {
		h.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}

	go h.luaService.RunCronScript(r.Context(), req.Script)

	h.handler.sendJSON(w, map[string]string{"status": "started"})
}
