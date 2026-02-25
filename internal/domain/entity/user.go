package entity

type User struct {
	JID  string
	LID  string
	Role string
}

type Group struct {
	JID          string
	Name         string
	Participants []*Participant
}

type Participant struct {
	JID string
	LID string
}

type UserInfo struct {
	JID string
	LID string
}
