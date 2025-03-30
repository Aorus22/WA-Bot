package utils

import (
	"fmt"
	"os"
	"strings"

	"go.mau.fi/whatsmeow"
	waTypes "go.mau.fi/whatsmeow/types"
)

func AssignRole(client *whatsmeow.Client, isFromGroup bool, senderJID waTypes.JID) string {
	owner := os.Getenv("OWNER_JID")
	if senderJID.String() == owner {
		return "OWNER"
	}

	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	if isFromGroup {
		if Contains(adminGroups, senderJID.String()) {
			return "ADMIN"
		} else if Contains(adminGroups, senderJID.String()) {
			return "USER"
		}
	}

	isAdmin := false
	for _, adminGroup := range adminGroups {
		targetGroupJID, err := waTypes.ParseJID(adminGroup)
		if err != nil {
			fmt.Println("Invalid group JID:", err)
			continue
		}

		groupInfo, err := client.GetGroupInfo(targetGroupJID)
		if err != nil {
			fmt.Println("Failed to get group info for", adminGroup, ":", err)
			continue
		}

		for _, participant := range groupInfo.Participants {
			if participant.JID.String() == senderJID.String() {
				isAdmin = true
				break
			}
		}

		if isAdmin {
			return "ADMIN"
		}
	}

	isUser := false
	for _, userGroup := range userGroups {
		targetGroupJID, err := waTypes.ParseJID(userGroup)
		if err != nil {
			fmt.Println("Invalid group JID:", err)
			continue
		}

		groupInfo, err := client.GetGroupInfo(targetGroupJID)
		if err != nil {
			fmt.Println("Failed to get group info for", userGroup, ":", err)
			continue
		}

		for _, participant := range groupInfo.Participants {
			if participant.JID.String() == senderJID.String() {
				isUser = true
				break
			}
		}

		if isUser {
			return "USER"
		}
	}

	return "COMMON"
}

func IsFromAllowedGroups(vInfo *waTypes.MessageInfo) bool {
	adminGroups := strings.Split(os.Getenv("ADMIN_GROUPS_JID"), ",")
	groupJID := vInfo.Chat.String()

	return Contains(adminGroups, groupJID)
}