package adminHandlers

import (
	"context"
	"fmt"

	"wa-bot/state"
	"wa-bot/utils"
)

func ListgroupsHandler(s *state.MessageState){
	if s.UserRole != "OWNER" {
		s.Reply("Invalid Command")
		return
	}

	groups, err := s.Client.GetJoinedGroups()
	if err != nil {
		fmt.Println("Error fetching joined groups:", err)
		return
	}

	responseText := "📌 *Daftar Grup:*\n\n"
	for _, group := range groups {
		responseText += fmt.Sprintf("📂 *%s*\n📎 ID: %s\n", group.Name, group.JID.String())

		_, err :=  s.Client.GetGroupInfo(group.JID)
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
	}

	var listMapelString string
	for i, mapel := range listMapel {
		listMapelString += fmt.Sprintf("%d. %s\n", i+1, mapel)
	}

	s.Reply(listMapelString)
}