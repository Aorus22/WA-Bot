package context

import (
	"fmt"
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

func (us *UserStateType) CancelUser(senderJID string) error {
	us.Lock()
	defer us.Unlock()

	data, exists := us.Data[senderJID]
	if !exists || data.Cancel == nil {
		return fmt.Errorf("User not found or no cancel function available")
	}

	data.Cancel()
	delete(us.Data, senderJID)
	return nil
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