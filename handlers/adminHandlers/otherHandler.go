package adminHandlers

import (
	"fmt"

	"wa-bot/context"
	"wa-bot/utils"
)

func ListgroupsHandler(ctx *context.MessageContext){
	if ctx.UserRole != "OWNER" {
		ctx.Reply("Invalid Command")
		return
	}

	groups, err := ctx.Client.GetJoinedGroups()
	if err != nil {
		fmt.Println("Error fetching joined groups:", err)
		return
	}

	responseText := "📌 *Daftar Grup:*\n\n"
	for _, group := range groups {
		responseText += fmt.Sprintf("📂 *%s*\n📎 ID: %s\n", group.Name, group.JID.String())

		_, err :=  ctx.Client.GetGroupInfo(group.JID)
		if err != nil {
			fmt.Println("Failed to get group info for", group.JID.String(), ":", err)
			continue
		}
	}

	ctx.Reply(responseText)
}

func ListMapelHandler(ctx *context.MessageContext) {
	isAllowed := ctx.UserRole == "ADMIN" || ctx.UserRole == "OWNER"

	if !isAllowed {
		return
	}

	listMapel, err := utils.FetchMapel()
	if err != nil {
		fmt.Println("Failed to fetch mapel:", err)
		return
	}

	var listMapelString string
	for i, mapel := range listMapel {
		listMapelString += fmt.Sprintf("%d. %s\n", i+1, mapel)
	}

	ctx.Reply(listMapelString)
}