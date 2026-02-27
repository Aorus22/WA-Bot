package entity

import "time"

type Trigger struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Pattern   string    `json:"pattern"`
	Script    string    `json:"script"`
	IsActive  bool      `json:"is_active"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}
