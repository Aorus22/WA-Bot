package commonHandlers

import (
	"strings"
	"wa-bot/state"
)

func CheckHandler(s *state.MessageState) {
	s.Reply("Hello, World!")
}

func GetCommandListHandler(s *state.MessageState) {
	role := s.UserRole
	var message string

	switch role {
	case "COMMON":
		message = strings.TrimSpace(`
			COMMANDS LIST

			_From URL:_
			- ` + "`!sticker`" + ` <video/gif/image URL>

			_Send with image/video/gif:_
			- ` + "`!sticker`" + `

			_Optional parameters_ (can be added after the command or URL):
			- ` + "`nocrop`" + ` // Prevent auto-cropping to square
			- ` + "`start=MM:SS`" + ` // Start time for video/gif
			- ` + "`end=MM:SS`" + ` // End time for video/gif
			- ` + "`fps=N`" + ` // Frame per second (1-60)
			- ` + "`quality=N`" + ` // Output quality (1-100)
			- ` + "`direction=side`" + ` // Pan direction: up, down, left, right
			- ` + "`direction=side-N`" + ` // Pan with offset (0-50), e.g., ` + "`right-25`" + `

			*Examples:*
			1. !sticker https://demo.alyza.site nocrop start=00:00 end=00:02 fps=24 quality=80
			2. !sticker https://demo.alyza.site/ direction=left-30 quality=90
		`)

	case "USER":
		message = strings.TrimSpace(`
			*LIST COMMANDS*
			1. ` + "`!token`" + `
		`)
	case "ADMIN":
		message = strings.TrimSpace(`
			*LIST COMMANDS*
			1. ` + "`!listmapel`" + `
			2. ` + "`!pdf <nomor dari !listmapel>`" + `
			3. ` + "`!pdf <nama mapel>`" + `
			4. ` + "`!answer <nomor dari !listmapel <jawaban>`" + `
			5. ` + "`!answer <nama mapel> <jawaban>`" + `
		`)
	case "OWNER":
		message = strings.TrimSpace(`
			*LIST COMMANDS*

			*ADMIN*
			1. ` + "`!listmapel`" + `
			2. ` + "`!pdf <nomor dari !listmapel>`" + `
			3. ` + "`!pdf <nama mapel>`" + `
			4. ` + "`!answer <nomor dari !listmapel <jawaban>`" + `
			5. ` + "`!answer <nama mapel> <jawaban>`" + `

			*USER*
			1. ` + "`!token`" + `

			*COMMON*
			_From URL:_
			- ` + "`!sticker <video/gif/image URL>`" + `

			_Send with image/video/gif:_
			- ` + "`!sticker`" + `

			_Optional parameters_ (can be added after the command or URL):
			- ` + "`nocrop`" + ` // Prevent auto-cropping to square
			- ` + "`start=MM:SS`" + ` // Start time for video/gif
			- ` + "`end=MM:SS`" + ` // End time for video/gif
			- ` + "`fps=N`" + ` // Frame per second (1-60)
			- ` + "`quality=N`" + ` // Output quality (1-100)
			- ` + "`direction=side`" + ` // Pan direction: up, down, left, right
			- ` + "`direction=side-N`" + ` // Pan with offset (0-50), e.g., ` + "`right-25`" + `

			*Examples:*
			1. !sticker https://demo.alyza.site nocrop start=00:00 end=00:02 fps=24 quality=80
			2. !sticker https://demo.alyza.site/ direction=left-30 quality=90
		`)
	}

	lines := strings.Split(message, "\n")
	for i := range lines {
		lines[i] = strings.TrimLeft(lines[i], "\t ")
	}
	message = strings.Join(lines, "\n")

	s.Reply(message)
}

func CancelHandler(s *state.MessageState) {
	state := s.CheckUserState()
	if state == "" {
		s.Reply("❌ There is no running process")
		return
	}

	err := s.CancelCurrentProcess()
	if err != nil {
		s.Reply("⚠️ Failed to cancel process")
	}

	s.Reply("✅ Process successfully cancelled")
}
