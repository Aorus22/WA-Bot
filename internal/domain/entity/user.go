package entity

type User struct {
	ID   string
	Name string
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
