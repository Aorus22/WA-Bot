// Package account keeps WhatsApp data separated per linked phone number.
//
// Each account owns <root>/<phone>/database/wa-bot-messages.db and
// <root>/<phone>/media. While nobody is linked (QR phase) a throwaway
// "_pending" directory is used, and wiped on every entry.
package account

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sync"
	"time"

	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/call"
	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"
)

const pendingDir = "_pending"

var phonePattern = regexp.MustCompile(`^[0-9A-Za-z_.-]+$`)

type mediaDir interface {
	SetBasePath(path string) error
}

type broadcaster interface {
	BroadcastMessage(msgType string, payload interface{})
}

// Manager switches the per-account data (message DB, media dir) and drives the
// logout -> new QR flow.
type Manager struct {
	root     string
	msgStore *repository.MessageStore
	media    mediaDir
	wa       *whatsappInfra.WhatsAppClient
	hub      broadcaster
	callSvc  *call.CallService

	// onSwitch hooks reset in-memory state that was loaded from the previous
	// account's database (e.g. history sync status).
	onSwitch []func()
	// startPairing begins a QR pairing session on the current client.
	startPairing func()

	mu    sync.Mutex
	phone string
}

func NewManager(root string, msgStore *repository.MessageStore, media mediaDir, wa *whatsappInfra.WhatsAppClient, hub broadcaster, callSvc *call.CallService) *Manager {
	return &Manager{root: root, msgStore: msgStore, media: media, wa: wa, hub: hub, callSvc: callSvc}
}

// OnSwitch registers a callback run after the data directory changed.
func (m *Manager) OnSwitch(fn func()) { m.onSwitch = append(m.onSwitch, fn) }

// SetPairingStarter registers the function that starts QR pairing.
func (m *Manager) SetPairingStarter(fn func()) { m.startPairing = fn }

// Phone returns the currently bound account number ("" while unlinked).
func (m *Manager) Phone() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.phone
}

// BindAccount points message DB and media at the given phone number's folder.
func (m *Manager) BindAccount(phone string) error {
	if !phonePattern.MatchString(phone) {
		return fmt.Errorf("invalid account id %q", phone)
	}
	if err := m.bind(phone, filepath.Join(m.root, phone)); err != nil {
		return err
	}
	m.removeStalePending("")
	return nil
}

// BindPending points the stores at an empty throwaway directory (nobody linked).
func (m *Manager) BindPending() error {
	// A fresh directory each time: the previous one may still be open (and
	// cannot be deleted on Windows) until the stores switched away from it.
	dir := filepath.Join(m.root, fmt.Sprintf("%s-%d", pendingDir, time.Now().UnixNano()))
	if err := m.bind("", dir); err != nil {
		return err
	}
	m.removeStalePending(dir)
	return nil
}

func (m *Manager) removeStalePending(keep string) {
	matches, _ := filepath.Glob(filepath.Join(m.root, pendingDir+"*"))
	for _, p := range matches {
		if p != keep {
			_ = os.RemoveAll(p)
		}
	}
}

func (m *Manager) bind(phone, dir string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if err := m.msgStore.Switch(filepath.Join(dir, "database", "wa-bot-messages.db")); err != nil {
		return err
	}
	if err := m.media.SetBasePath(filepath.Join(dir, "media")); err != nil {
		return fmt.Errorf("failed to switch media directory: %w", err)
	}
	m.phone = phone
	for _, fn := range m.onSwitch {
		fn()
	}
	fmt.Printf("[ACCOUNT] Using data of %q (%s)\n", phone, dir)
	return nil
}

// HandlePaired is called on events.PairSuccess, before any history arrives.
func (m *Manager) HandlePaired(phone string) {
	if err := m.BindAccount(phone); err != nil {
		fmt.Printf("[ACCOUNT] Failed to bind account %s: %v\n", phone, err)
	}
}

// Logout unlinks the current device and prepares a fresh QR for another account.
func (m *Manager) Logout() error {
	if err := m.wa.Logout(); err != nil {
		return err
	}
	m.HandleLoggedOut()
	return nil
}

// HandleLoggedOut resets everything tied to the old account and starts a new
// pairing session. Also used when the phone unlinks the device (events.LoggedOut).
func (m *Manager) HandleLoggedOut() {
	m.mu.Lock()
	alreadyUnlinked := m.phone == ""
	m.mu.Unlock()
	if alreadyUnlinked {
		// Logout() already reset everything; this is the trailing server event.
		return
	}
	if err := m.BindPending(); err != nil {
		fmt.Printf("[ACCOUNT] Failed to reset account data: %v\n", err)
	}
	m.wa.ResetSession()
	if m.callSvc != nil {
		m.callSvc.SetClient(m.wa.GetCallClient())
	}
	m.hub.BroadcastMessage("logged_out", nil)
	if m.startPairing != nil {
		go m.startPairing()
	}
}
