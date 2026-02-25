package valueobject

import "strings"

type JID string

func NewJID(s string) JID {
	return JID(s)
}

func (j JID) String() string {
	return string(j)
}

func (j JID) IsEmpty() bool {
	return strings.TrimSpace(j.String()) == ""
}

func (j JID) User() string {
	parts := strings.Split(j.String(), "@")
	if len(parts) > 0 {
		return parts[0]
	}
	return ""
}
