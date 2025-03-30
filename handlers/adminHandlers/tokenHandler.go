package adminHandlers

import (
	"fmt"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"

	"wa-bot/context"
	"wa-bot/utils"
)

func TokenHandler(ctx *context.MessageContext) {
	isAllowed := ctx.UserRole == "ADMIN" || ctx.UserRole == "OWNER" || ctx.UserRole == "USER"
	isFromGroup := ctx.IsFromGroup

	if !isAllowed || isFromGroup {
		ctx.Reply("Invalid Command")
		return
	}

	ctx.AddUserToState("PendingToken");
	ctx.Reply("Silakan masukkan nama lengkap Anda.")
}

func GetNameHandler(ctx *context.MessageContext) {

	ctx.Reply("⏳ Loading...")

	timeoutStr := os.Getenv("TIMEOUT_NAMA")

	timeout, err := strconv.Atoi(timeoutStr)
	if err != nil {
		timeout = 2
	}

	startTime, err := ctx.GetUserPendingStartTime()

	if err != nil {
		fmt.Println("User not Found in State", err)
		return
	}

	if time.Since(startTime) > time.Duration(timeout)*time.Minute {
		ctx.ClearUserState()
		ctx.Reply("⏳ Waktu habis! Silakan ketik *!token* lagi.")
		return
	}

	var validNameRegex = regexp.MustCompile(`^[a-zA-Z' ]+$`)

	if !validNameRegex.MatchString(ctx.MessageText) {
		ctx.Reply("⚠️ Nama Invalid")
		ctx.ClearUserState()
		return
	}

	nis := strings.Split(ctx.SenderJID.String(), "@")[0]
	nama := ctx.MessageText

	ctx.ClearUserState()

	status, token, err := utils.FetchTokenData(nama, nis)
	if err != nil {
		fmt.Println("Failed to fetch token:", err)
		return
	}

	var responseText string
	if status == "new" {
		responseText = "✅ Token baru Anda adalah:"
	} else if status == "update" {
		responseText = "Token lama telah tidak berlaku. Ini token baru anda:"
	} else {
		responseText = "Gagal mendapatkan token."
	}

	ctx.Reply(responseText)
	ctx.Reply(token)
}
