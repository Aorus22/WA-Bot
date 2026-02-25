package entity

import "time"

type UserState struct {
	JID       string
	Status    string
	StartTime time.Time
	Cancel    func()
}
