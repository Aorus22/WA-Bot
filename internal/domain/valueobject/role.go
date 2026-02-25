package valueobject

type Role string

const (
	RoleOwner  Role = "OWNER"
	RoleAdmin  Role = "ADMIN"
	RoleUser   Role = "USER"
	RoleCommon Role = "COMMON"
)

func (r Role) String() string {
	return string(r)
}

func (r Role) IsValid() bool {
	switch r {
	case RoleOwner, RoleAdmin, RoleUser, RoleCommon:
		return true
	default:
		return false
	}
}

func (r Role) CanAccessAdmin() bool {
	return r == RoleOwner || r == RoleAdmin
}

func (r Role) CanAccessUser() bool {
	return r == RoleOwner || r == RoleAdmin || r == RoleUser
}

func (r Role) CanAccessCommon() bool {
	return r == RoleOwner || r == RoleCommon
}
