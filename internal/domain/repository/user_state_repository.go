package repository

import (
	"wa-bot/internal/domain/entity"
)

type UserStateRepository interface {
	AddUser(jid, status string, cancel func())
	ClearUser(jid string)
	GetUserStatus(jid string) (*entity.UserState, error)
	CancelUser(jid string) error
	UpdateProcessContext(jid string, cancel func())
	SetUserState(jid, state string)
	ClearUserState(jid string)
	GetUserState(jid string) (string, error)
	GetUserStateSimple(jid string) string
	CancelUserState(jid string) error
}
