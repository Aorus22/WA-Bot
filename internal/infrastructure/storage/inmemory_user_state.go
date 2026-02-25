package storage

import (
	"wa-bot/internal/domain/entity"
)

type InMemoryUserState struct {
	data map[string]*entity.UserState
}

func NewInMemoryUserState() *InMemoryUserState {
	return &InMemoryUserState{
		data: make(map[string]*entity.UserState),
	}
}

func (i *InMemoryUserState) AddUser(jid, status string, cancel func()) {
	i.data[jid] = &entity.UserState{
		JID:       jid,
		Status:    status,
		StartTime: entity.StartTime(),
		Cancel:    cancel,
	}
}

func (i *InMemoryUserState) ClearUser(jid string) {
	delete(i.data, jid)
}

func (i *InMemoryUserState) GetUserStatus(jid string) (*entity.UserState, error) {
	if data, exists := i.data[jid]; exists {
		return data, nil
	}
	return nil, nil
}

func (i *InMemoryUserState) CancelUser(jid string) error {
	if data, exists := i.data[jid]; exists {
		if data.Cancel != nil {
			data.Cancel()
		}
		delete(i.data, jid)
		return nil
	}
	return nil
}

func (i *InMemoryUserState) UpdateProcessContext(jid string, cancel func()) {
	if data, exists := i.data[jid]; exists {
		data.Cancel = cancel
	}
}

func (i *InMemoryUserState) SetUserState(jid, state string) {
	i.data[jid] = &entity.UserState{
		JID:    jid,
		Status: state,
	}
}

func (i *InMemoryUserState) ClearUserState(jid string) {
	delete(i.data, jid)
}

func (i *InMemoryUserState) GetUserState(jid string) (string, error) {
	if data, exists := i.data[jid]; exists {
		return data.Status, nil
	}
	return "", nil
}

func (i *InMemoryUserState) GetUserStateSimple(jid string) string {
	if data, exists := i.data[jid]; exists {
		return data.Status
	}
	return ""
}

func (i *InMemoryUserState) CancelUserState(jid string) error {
	if data, exists := i.data[jid]; exists {
		if data.Cancel != nil {
			data.Cancel()
		}
		delete(i.data, jid)
		return nil
	}
	return nil
}
