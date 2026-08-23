package ui

import "github.com/diamondburned/gotk4/pkg/glib/v2"

// Dispatcher schedules a function to run on the GTK main thread.
// All UI updates from goroutines (HTTP callbacks, file watchers, etc.) MUST
// go through this helper. Direct widget access from non-main goroutines is
// undefined behaviour in GTK4.
//
// Usage:
//
//	go func() {
//	    result, err := doWork()
//	    dispatcher.OnMainThread(func() {
//	        updateLabel(result, err)
//	    })
//	}()
type Dispatcher struct{}

// NewDispatcher returns a stateless dispatcher. The zero value is also usable.
func NewDispatcher() Dispatcher { return Dispatcher{} }

// OnMainThread schedules fn to run on the GTK main thread during the next
// idle phase. The function runs once; IdleAdd returns G_SOURCE_REMOVE.
func (Dispatcher) OnMainThread(fn func()) {
	glib.IdleAdd(func() bool {
		fn()
		return false
	})
}
