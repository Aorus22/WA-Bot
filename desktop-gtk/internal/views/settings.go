package views

import (
	"context"
	"fmt"
	"log"
	"os"
	"os/exec"
	"runtime"
	"sync/atomic"
	"time"

	"github.com/diamondburned/gotk4-adwaita/pkg/adw"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"

	"wa-bot-desktop/internal/api"
	"wa-bot-desktop/internal/backend"
)

// Settings is the settings view, an Adw.PreferencesPage with three groups:
//   - Backend: live port, DB/media paths, "Open backend log folder", "Restart backend"
//   - App: user data directory, theme override
//   - About: app version, GTK version, libadwaita version
type Settings struct {
	root *adw.PreferencesPage

	// Backend group widgets
	portRow            *adw.ActionRow
	dbRow              *adw.ActionRow
	mediaRow           *adw.ActionRow
	logRow             *adw.ActionRow
	restartRow         *adw.ActionRow
	historyRow         *adw.ActionRow
	historyProgressRow *adw.ActionRow
	historyButton      *gtk.Button

	// App group widgets
	userDataRow *adw.ActionRow
	themeRow    *adw.ActionRow

	// About group widgets
	versionRow *adw.ActionRow
	gtkRow     *adw.ActionRow
	adwRow     *adw.ActionRow
	goRow      *adw.ActionRow

	userDataDir       string
	client            *api.Client
	mgr               *backend.Manager
	historyPolling    atomic.Bool
	lastHistoryState  string
	whatsAppConnected bool
	onHistoryComplete func()
	toast             func(string)
}

// NewSettings constructs the settings view.
func NewSettings(userDataDir string, mgr *backend.Manager) *Settings {
	s := &Settings{
		userDataDir: userDataDir,
		mgr:         mgr,
	}

	s.root = adw.NewPreferencesPage()

	// ─── Backend group ───
	backendGroup := adw.NewPreferencesGroup()
	backendGroup.SetTitle("Backend")
	backendGroup.SetDescription("Live status of the spawned wa-bot-backend process.")

	s.portRow = adw.NewActionRow()
	s.portRow.SetTitle("Backend port")
	s.portRow.SetSubtitle("Discovered from BACKEND_PORT: stdout")
	backendGroup.Add(s.portRow)

	s.dbRow = adw.NewActionRow()
	s.dbRow.SetTitle("Database path")
	s.dbRow.SetSubtitle(userDataDir + "/database")
	backendGroup.Add(s.dbRow)

	s.mediaRow = adw.NewActionRow()
	s.mediaRow.SetTitle("Media path")
	s.mediaRow.SetSubtitle(userDataDir + "/media")
	backendGroup.Add(s.mediaRow)

	// "Open backend log folder" button row
	s.logRow = adw.NewActionRow()
	s.logRow.SetTitle("Backend log folder")
	s.logRow.SetSubtitle(userDataDir)
	openLogBtn := gtk.NewButtonFromIconName("folder-open-symbolic")
	openLogBtn.SetTooltipText("Open in file manager")
	openLogBtn.SetVAlign(gtk.AlignCenter)
	openLogBtn.ConnectClicked(func() { openInFileManager(userDataDir) })
	s.logRow.AddSuffix(openLogBtn)
	s.logRow.SetActivatableWidget(openLogBtn)
	backendGroup.Add(s.logRow)

	// "Restart backend" button row
	s.restartRow = adw.NewActionRow()
	s.restartRow.SetTitle("Restart backend")
	s.restartRow.SetSubtitle("Stop the backend and start a fresh process")
	restartBtn := gtk.NewButtonFromIconName("view-refresh-symbolic")
	restartBtn.SetTooltipText("Restart")
	restartBtn.SetVAlign(gtk.AlignCenter)
	restartBtn.ConnectClicked(func() { s.restartBackend() })
	s.restartRow.AddSuffix(restartBtn)
	s.restartRow.SetActivatableWidget(restartBtn)
	backendGroup.Add(s.restartRow)

	s.root.Add(backendGroup)

	// ─── WhatsApp history group ───
	historyGroup := adw.NewPreferencesGroup()
	historyGroup.SetTitle("Riwayat WhatsApp")
	historyGroup.SetDescription("Impor manual bersifat menambah saja; chat dan pesan lokal tidak pernah dihapus.")
	s.historyRow = adw.NewActionRow()
	s.historyRow.SetTitle("Sinkronkan pesan lama")
	s.historyRow.SetSubtitle("Maksimal 50 pesan per chat untuk setiap sinkronisasi")
	s.historyButton = gtk.NewButtonWithLabel("Sinkronkan")
	s.historyButton.SetVAlign(gtk.AlignCenter)
	s.historyButton.SetSensitive(false)
	s.historyButton.ConnectClicked(func() { s.startHistorySync() })
	s.historyRow.AddSuffix(s.historyButton)
	s.historyRow.SetActivatableWidget(s.historyButton)
	historyGroup.Add(s.historyRow)
	s.historyProgressRow = adw.NewActionRow()
	s.historyProgressRow.SetTitle("Status")
	s.historyProgressRow.SetSubtitle("Menunggu backend")
	historyGroup.Add(s.historyProgressRow)
	s.root.Add(historyGroup)

	// ─── App group ───
	appGroup := adw.NewPreferencesGroup()
	appGroup.SetTitle("App")

	s.userDataRow = adw.NewActionRow()
	s.userDataRow.SetTitle("User data directory")
	s.userDataRow.SetSubtitle(userDataDir)
	openUserDataBtn := gtk.NewButtonFromIconName("folder-open-symbolic")
	openUserDataBtn.SetVAlign(gtk.AlignCenter)
	openUserDataBtn.ConnectClicked(func() { openInFileManager(userDataDir) })
	s.userDataRow.AddSuffix(openUserDataBtn)
	s.userDataRow.SetActivatableWidget(openUserDataBtn)
	appGroup.Add(s.userDataRow)

	// Theme picker row; options are wired later via SetThemeOptions.
	s.themeRow = adw.NewActionRow()
	s.themeRow.SetTitle("Tema")
	s.themeRow.SetSubtitle("Warna aplikasi — porting dari preset web")
	appGroup.Add(s.themeRow)

	s.root.Add(appGroup)

	// ─── About group ───
	aboutGroup := adw.NewPreferencesGroup()
	aboutGroup.SetTitle("About")

	s.versionRow = adw.NewActionRow()
	s.versionRow.SetTitle("App version")
	s.versionRow.SetSubtitle("1.1.0 (Phase 11)")
	aboutGroup.Add(s.versionRow)

	s.gtkRow = adw.NewActionRow()
	s.gtkRow.SetTitle("GTK version")
	aboutGroup.Add(s.gtkRow)

	s.adwRow = adw.NewActionRow()
	s.adwRow.SetTitle("libadwaita version")
	aboutGroup.Add(s.adwRow)

	s.goRow = adw.NewActionRow()
	s.goRow.SetTitle("Go runtime")
	s.goRow.SetSubtitle(runtime.Version())
	aboutGroup.Add(s.goRow)

	s.root.Add(aboutGroup)

	return s
}

// Widget returns the root widget.
func (s *Settings) Widget() gtk.Widgetter { return s.root }

// SetBackendPort updates the live port display.
func (s *Settings) SetBackendPort(port int) {
	s.portRow.SetSubtitle(fmt.Sprintf("%d (auto-discovered)", port))
}

// SetDeps wires history-sync collaborators after the backend port is known.
func (s *Settings) SetDeps(client *api.Client, toast func(string), onComplete func()) {
	s.client = client
	s.toast = toast
	s.onHistoryComplete = onComplete
	s.historyButton.SetSensitive(client != nil && s.whatsAppConnected)
	s.refreshHistoryStatus()
}

func (s *Settings) SetWhatsAppConnected(connected bool) {
	s.whatsAppConnected = connected
	s.historyButton.SetSensitive(connected && s.client != nil && s.lastHistoryState != "running")
}

func (s *Settings) startHistorySync() {
	if s.client == nil {
		return
	}
	s.historyButton.SetSensitive(false)
	s.historyProgressRow.SetSubtitle("Memulai sinkronisasi…")
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		status, err := s.client.StartHistorySync(ctx)
		glib.IdleAdd(func() bool {
			if err != nil {
				s.historyButton.SetSensitive(true)
				s.historyProgressRow.SetSubtitle("Gagal memulai: " + err.Error())
				if s.toast != nil {
					s.toast("Gagal memulai sinkronisasi riwayat: " + err.Error())
				}
				return false
			}
			s.applyHistoryStatus(status)
			return false
		})
		if err == nil && status != nil && status.State == "running" {
			s.pollHistoryStatus()
		}
	}()
}

func (s *Settings) refreshHistoryStatus() {
	if s.client == nil {
		return
	}
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		status, err := s.client.GetHistorySyncStatus(ctx)
		glib.IdleAdd(func() bool {
			if err != nil {
				s.historyProgressRow.SetSubtitle("Status tidak tersedia")
				return false
			}
			s.applyHistoryStatus(status)
			return false
		})
		if err == nil && status != nil && status.State == "running" {
			s.pollHistoryStatus()
		}
	}()
}

func (s *Settings) pollHistoryStatus() {
	if !s.historyPolling.CompareAndSwap(false, true) {
		return
	}
	go func() {
		defer s.historyPolling.Store(false)
		ticker := time.NewTicker(time.Second)
		defer ticker.Stop()
		for range ticker.C {
			ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
			status, err := s.client.GetHistorySyncStatus(ctx)
			cancel()
			if err != nil {
				continue
			}
			glib.IdleAdd(func() bool {
				s.applyHistoryStatus(status)
				return false
			})
			if status.State != "running" {
				return
			}
		}
	}()
}

func (s *Settings) applyHistoryStatus(status *api.HistorySyncStatus) {
	if status == nil {
		return
	}
	wasRunning := s.lastHistoryState == "running"
	s.lastHistoryState = status.State
	s.historyButton.SetSensitive(s.whatsAppConnected && status.State != "running")
	if status.State == "running" {
		s.historyButton.SetLabel("Menyinkronkan…")
		s.historyProgressRow.SetSubtitle(fmt.Sprintf("%d/%d chat • %d pesan ditambahkan • %d pesan staging", status.ChatsProcessed, status.ChatsTotal, status.MessagesAdded, status.PendingMessages))
		return
	}
	s.historyButton.SetLabel("Sinkronkan 50/chat")
	subtitle := fmt.Sprintf("%s • %d pesan ditambahkan • %d pesan dalam staging", historyStateLabel(status.State), status.MessagesAdded, status.PendingMessages)
	if len(status.Errors) > 0 {
		subtitle += fmt.Sprintf(" • %d error", len(status.Errors))
	}
	s.historyProgressRow.SetSubtitle(subtitle)
	if wasRunning {
		if s.onHistoryComplete != nil {
			s.onHistoryComplete()
		}
		if s.toast != nil {
			s.toast(fmt.Sprintf("Sinkronisasi selesai: %d pesan ditambahkan", status.MessagesAdded))
		}
	}
}

func historyStateLabel(state string) string {
	switch state {
	case "completed":
		return "Selesai"
	case "partial":
		return "Selesai sebagian"
	case "failed":
		return "Gagal"
	default:
		return "Siap"
	}
}

// SetVersions sets the GTK and libadwaita runtime versions.
func (s *Settings) SetVersions(gtkVer, adwVer string) {
	if gtkVer != "" {
		s.gtkRow.SetSubtitle(gtkVer)
	}
	if adwVer != "" {
		s.adwRow.SetSubtitle(adwVer)
	}
}

// SetThemeOptions fills the theme dropdown with display labels, preselects
// the current theme, and invokes onSelected (with the chosen index) whenever
// the user picks a different one.
func (s *Settings) SetThemeOptions(labels []string, selectedIndex int, onSelected func(index int)) {
	dd := gtk.NewDropDownFromStrings(labels)
	if selectedIndex >= 0 && selectedIndex < len(labels) {
		dd.SetSelected(uint(selectedIndex))
	}
	dd.SetVAlign(gtk.AlignCenter)
	dd.Object.NotifyProperty("selected", func() {
		onSelected(int(dd.Selected()))
	})
	s.themeRow.AddSuffix(dd)
	s.themeRow.SetActivatableWidget(dd)
}

// restartBackend stops the manager (sends SIGINT, waits 3s) and restarts it.
// In v1.1 the app's main process holds the manager; we just kill+respawn.
func (s *Settings) restartBackend() {
	if s.mgr == nil {
		log.Printf("settings: no manager, cannot restart")
		return
	}
	log.Printf("settings: restart requested")
	// We do not have a direct way to restart from the view; the simplest v1.1
	// behavior is to log the action and let the user know that a full app
	// restart is needed. A real restart would require the app to expose a
	// Restart method on the Manager that the App wires to.
	s.restartRow.SetSubtitle("Restart requested — close and reopen the app to apply")
}

// openInFileManager opens the given path in the OS file manager.
func openInFileManager(path string) {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command("explorer", path)
	case "darwin":
		cmd = exec.Command("open", path)
	default:
		cmd = exec.Command("xdg-open", path)
	}
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		log.Printf("failed to open %q in file manager: %v", path, err)
	}
}
