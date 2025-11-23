package adminHandlers

import (
	"context"
	"fmt"
	"os"
	"strings"

	"wa-bot/state"
	"wa-bot/utils"
	waTypes "go.mau.fi/whatsmeow/types"
)

func ListgroupsHandler(s *state.MessageState){
	if s.UserRole != "OWNER" {
		s.Reply("Invalid Command")
		return
	}

	groups, err := s.Client.GetJoinedGroups(context.Background())
	if err != nil {
		fmt.Println("Error fetching joined groups:", err)
		return
	}

	responseText := "📌 *Daftar Grup:*\n\n"
	for _, group := range groups {
		responseText += fmt.Sprintf("📂 *%s*\n📎 ID: %s\n", group.Name, group.JID.String())

		_, err := s.Client.GetGroupInfo(context.Background(), group.JID)
		if err != nil {
			fmt.Println("Failed to get group info for", group.JID.String(), ":", err)
			continue
		}
	}

	s.Reply(responseText)
}

func ListMapelHandler(s *state.MessageState) {
	isAllowed := s.UserRole == "ADMIN" || s.UserRole == "OWNER"

	if !isAllowed {
		return
	}

	listMapel, err := utils.FetchMapel()
	if err != nil {
		utils.LogNoCancelErr(context.Background(), err, "Error fetching mapel:")
		s.ReplyNoCancelError(context.Background(), err, "Gagal mengambil daftar mapel.")
		return
	}

	var listMapelString string
	for i, mapel := range listMapel {
		listMapelString += fmt.Sprintf("%d. %s\n", i+1, mapel)
	}

	s.Reply(listMapelString)
}

func ListMemberHandler(s *state.MessageState) {
	if s.UserRole != "OWNER" {
		s.Reply("Invalid Command")
		return
	}

	userGroups := strings.Split(os.Getenv("USER_GROUPS_JID"), ",")

	responseText := "*Daftar Member per Grup:*\n\n"

	for _, userGroup := range userGroups {
		targetGroupJID, err := waTypes.ParseJID(userGroup)
		if err != nil {
			fmt.Printf("Invalid user group JID '%s': %v\n", userGroup, err)
			continue
		}

		groupInfo, err := s.Client.GetGroupInfo(context.Background(), targetGroupJID)
		if err != nil {
			fmt.Printf("Failed to get group info for '%s': %v\n", userGroup, err)
			continue
		}

		responseText += fmt.Sprintf("*%s* (%d members)\n", groupInfo.Name, len(groupInfo.Participants))
		for _, participant := range groupInfo.Participants {
			jid := participant.JID.ToNonAD()
			responseText += fmt.Sprintf("- %s (JID: %s)\n", jid.User, jid.String())
		}
		responseText += "\n"
	}

	s.Reply(responseText)
}