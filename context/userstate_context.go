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
	Status    string
	StartTime time.Time
}

var UserState = UserStateType{
	Data: make(map[string]UserData),
}

func (us *UserStateType) AddUser(senderJID string, status string) {
	us.Lock()
	us.Data[senderJID] = UserData{
		Status:    status,
		StartTime: time.Now(),
	}
	us.Unlock()
}

func (us *UserStateType) ClearUser(senderJID string) {
	us.Lock()
	delete(us.Data, senderJID)
	us.Unlock()
}

func (us *UserStateType) GetUserStatus(senderJID string) (UserData, bool) {
	us.Lock()
	defer us.Unlock()
	data, exists := us.Data[senderJID]
	return data, exists
}