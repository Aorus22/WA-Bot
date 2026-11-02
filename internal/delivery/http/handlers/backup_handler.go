package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"strings"
	"time"
)

// BackupStatus is the progress of the latest backup run.
type BackupStatus struct {
	Running bool   `json:"running"`
	Account string `json:"account,omitempty"`
	// AccountIndex is 1-based; AccountCount is how many accounts this run covers.
	AccountIndex int `json:"accountIndex,omitempty"`
	AccountCount int `json:"accountCount,omitempty"`
	// Percent is the whole run, 0 to 100.
	Percent    int    `json:"percent"`
	Stage      string `json:"stage,omitempty"`
	Item       string `json:"item,omitempty"`
	Done       int64  `json:"done"`
	Total      int64  `json:"total"`
	StartedAt  int64  `json:"startedAt,omitempty"`
	FinishedAt int64  `json:"finishedAt,omitempty"`
	Summary    string `json:"summary,omitempty"`
	Error      string `json:"error,omitempty"`
}

// BackupEntry is one backup found in a target folder.
type BackupEntry struct {
	Account   string `json:"account"`
	ID        string `json:"id"`
	CreatedAt int64  `json:"createdAt"`
}

// BackupService runs backups into a local folder. Implemented by the binary,
// which knows where the accounts live.
type BackupService interface {
	Start(target string, keep int) error
	Status() BackupStatus
	List(ctx context.Context, target string) ([]BackupEntry, error)
	// StartRestore merges a backup into the account's live data; nothing live
	// is replaced.
	StartRestore(target, account, id string) error
	RestoreStatus() BackupStatus
}

func (h *Handler) SetBackupService(s BackupService) { h.backup = s }

type BackupHandler struct{ handler *Handler }

func NewBackupHandler(h *Handler) *BackupHandler { return &BackupHandler{handler: h} }

func (bh *BackupHandler) service(w http.ResponseWriter) BackupService {
	if bh.handler.backup == nil {
		bh.handler.sendError(w, http.StatusNotImplemented, "backup is not available")
		return nil
	}
	return bh.handler.backup
}

// Run starts a backup in the background; poll Status for progress.
func (bh *BackupHandler) Run(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusOK)
		return
	}
	svc := bh.service(w)
	if svc == nil {
		return
	}
	var req struct {
		Target string `json:"target"`
		Keep   int    `json:"keep"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		bh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := svc.Start(strings.TrimSpace(req.Target), req.Keep); err != nil {
		bh.handler.sendError(w, http.StatusConflict, err.Error())
		return
	}
	bh.handler.sendSuccess(w, nil)
}

func (bh *BackupHandler) Status(w http.ResponseWriter, r *http.Request) {
	if svc := bh.service(w); svc != nil {
		bh.handler.sendJSON(w, svc.Status())
	}
}

func (bh *BackupHandler) List(w http.ResponseWriter, r *http.Request) {
	svc := bh.service(w)
	if svc == nil {
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()
	list, err := svc.List(ctx, strings.TrimSpace(r.URL.Query().Get("target")))
	if err != nil {
		bh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if list == nil {
		list = []BackupEntry{}
	}
	bh.handler.sendJSON(w, list)
}

// Restore merges one backup into the running app's data in the background;
// poll RestoreStatus for progress.
func (bh *BackupHandler) Restore(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusOK)
		return
	}
	svc := bh.service(w)
	if svc == nil {
		return
	}
	var req struct {
		Target  string `json:"target"`
		Account string `json:"account"`
		ID      string `json:"id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		bh.handler.sendError(w, http.StatusBadRequest, err.Error())
		return
	}
	if err := svc.StartRestore(strings.TrimSpace(req.Target), strings.TrimSpace(req.Account), strings.TrimSpace(req.ID)); err != nil {
		bh.handler.sendError(w, http.StatusConflict, err.Error())
		return
	}
	bh.handler.sendSuccess(w, nil)
}

func (bh *BackupHandler) RestoreStatus(w http.ResponseWriter, r *http.Request) {
	if svc := bh.service(w); svc != nil {
		bh.handler.sendJSON(w, svc.RestoreStatus())
	}
}
