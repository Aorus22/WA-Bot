package adminHandlers

import (
	"context"
	"fmt"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"

	"wa-bot/state"
	"wa-bot/utils"
)

func TokenHandler(s *state.MessageState) {
	isAllowed := s.UserRole == "ADMIN" || s.UserRole == "OWNER" || s.UserRole == "USER"
	isFromGroup := s.IsFromGroup

	if !isAllowed || isFromGroup {
		s.Reply("Invalid Command")
		return
	}

	s.AddUserToState("PendingToken", func() {});
	s.Reply("Silakan masukkan nama lengkap Anda.")
}

func GetNameHandler(s *state.MessageState) {
	s.Reply("⏳ Loading...")

	ctx, cancel := context.WithCancel(context.Background())
	s.UpdateUserProcess(cancel)

	go func() {
		defer s.ClearUserState()
		defer cancel()

		timeoutStr := os.Getenv("TIMEOUT_NAMA")

		timeout, err := strconv.Atoi(timeoutStr)
		if err != nil {
			timeout = 2
		}

		startTime, err := s.GetUserPendingStartTime()
		if err != nil {
			fmt.Println("Error getting start time:", err)
			return
		}

		if time.Since(startTime) > time.Duration(timeout)*time.Minute {
			s.Reply("⏳ Waktu habis! Silakan ketik *!token* lagi.")
			return
		}

		var validNameRegex = regexp.MustCompile(`^[a-zA-Z' ]+$`)

		if !validNameRegex.MatchString(s.MessageText) {
			s.Reply("⚠️ Nama Invalid")
			return
		}

		nis := strings.Split(s.SenderJID.String(), "@")[0]
		nama := s.MessageText

		status, token, err := utils.FetchTokenData(ctx, nama, nis)
		if err != nil {
			utils.LogNoCancelErr(ctx, err, "Error fetching token data:")
			s.ReplyNoCancelError(ctx, err, "Gagal mendapatkan token.")
			return
		}

		var responseText string
		if status == "new" {
			responseText = "✅ Token baru Anda adalah:"
		} else if status == "update" {
			responseText = "Token lama telah tidak berlaku. Ini token baru anda:"
		}

		if utils.IsCanceledGoroutine(ctx) { return }

		s.Reply(responseText)
		s.Reply(token)
	}()
}
