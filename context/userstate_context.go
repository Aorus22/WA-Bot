package context

import (
	"sync"
	"time"
)

type UserStateType struct {
	sync.Mutex
	Data map[string]UserData
}

type UserData struct {
	Status    		string
	StartTime 		time.Time
	Cancel 			func()
}

var UserState = UserStateType{
	Data: make(map[string]UserData),
}

func (us *UserStateType) AddUser(senderJID string, status string, cancelFunc ...func()) {
	us.Lock()
	defer us.Unlock()

	var cancel func()
	if len(cancelFunc) > 0 {
		cancel = cancelFunc[0]
	}

	us.Data[senderJID] = UserData{
		Status:    status,
		StartTime: time.Now(),
		Cancel:    cancel,
	}
}

func (us *UserStateType) ClearUser(senderJID string) {
	us.Lock()
	defer us.Unlock()

	delete(us.Data, senderJID)
}

func (us *UserStateType) GetUserStatus(senderJID string) (UserData, bool) {
	us.Lock()
	defer us.Unlock()

	data, exists := us.Data[senderJID]
	return data, exists
}

func (us *UserStateType) CancelUser(senderJID string) bool {
	us.Lock()
	defer us.Unlock()

	data, exists := us.Data[senderJID]
	if !exists || data.Cancel == nil {
		return false
	}

	data.Cancel()
	delete(us.Data, senderJID)
	return true
}

func (us *UserStateType) UpdateProcessContext (senderJID string, cancel func()) {
	us.Lock()
	defer us.Unlock()

	_, exists := us.Data[senderJID]
	if !exists {
		return
	}

	userData := us.Data[senderJID]
	userData.Cancel = cancel
	us.Data[senderJID] = userData
}