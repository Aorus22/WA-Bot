package utils

import (
	"context"
	"fmt"
	"os"
	"strings"

	"go.mau.fi/whatsmeow"
	waTypes "go.mau.fi/whatsmeow/types"
)

func AssignRole(client *whatsmeow.Client, isFromGroup bool, senderJID waTypes.JID) string {
	owner := os.Getenv("OWNER_JID")
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	var role string
	defer func() {
		fmt.Printf("[AssignRole] sender=%s | isFromGroup=%v | owner=%s | adminGroups=%v | userGroups=%v | result=%s\n",
			senderJID.String(), isFromGroup, owner, adminGroups, userGroups, role)
	}()

	if senderJID.String() == owner {
		role = "OWNER"
		return role
	}

	if isFromGroup {
		if Contains(adminGroups, senderJID.String()) {
			role = "ADMIN"
			return role
		} else if Contains(userGroups, senderJID.String()) {
			role = "USER"
			return role
		}
	}

	// Get sender's LID for comparison with group participants
	var senderLID waTypes.JID
	userInfo, err := client.GetUserInfo(context.Background(), []waTypes.JID{senderJID.ToNonAD()})
	if err == nil && len(userInfo) > 0 {
		for _, info := range userInfo {
			if !info.LID.IsEmpty() {
				senderLID = info.LID
			}
			break
		}
	}

	for _, adminGroup := range adminGroups {
		targetGroupJID, err := waTypes.ParseJID(adminGroup)
		if err != nil {
			continue
		}

		groupInfo, err := client.GetGroupInfo(context.Background(), targetGroupJID)
		if err != nil {
			continue
		}

		for _, participant := range groupInfo.Participants {
			participantJID := participant.JID.ToNonAD()

			// Handle LID format - compare with sender's LID
			if participantJID.Server == "lid" {
				if senderLID.User != "" && participantJID.User == senderLID.User {
					role = "ADMIN"
					return role
				}
				continue
			}

			// Normal phone number comparison
			if participantJID.String() == senderJID.ToNonAD().String() {
				role = "ADMIN"
				return role
			}
		}
	}

	for _, userGroup := range userGroups {
		targetGroupJID, err := waTypes.ParseJID(userGroup)
		if err != nil {
			continue
		}

		groupInfo, err := client.GetGroupInfo(context.Background(), targetGroupJID)
		if err != nil {
			continue
		}

		for _, participant := range groupInfo.Participants {
			participantJID := participant.JID.ToNonAD()

			// Handle LID format - compare with sender's LID
			if participantJID.Server == "lid" {
				if senderLID.User != "" && participantJID.User == senderLID.User {
					role = "USER"
					return role
				}
				continue
			}

			// Normal phone number comparison
			if participantJID.String() == senderJID.ToNonAD().String() {
				role = "USER"
				return role
			}
		}
	}

	role = "COMMON"
	return role
}

func IsFromAllowedGroups(vInfo *waTypes.MessageInfo) bool {
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	groupJID := vInfo.Chat.String()

	return Contains(adminGroups, groupJID)
}