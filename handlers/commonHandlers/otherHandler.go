package commonHandlers

import (
	"strings"
	"wa-bot/context"
)

func CheckHandler(ctx *context.MessageContext){
	ctx.Reply("Hello, World!")
}

func GetCommandList(ctx *context.MessageContext) {
	role := ctx.UserRole
	var message string

	switch role {
	case "COMMON":
		message = strings.TrimSpace(`
			COMMANDS LIST

			From Url:
			1. !sticker <video/gif/image URL>
			2. !sticker !crop <video/gif/image URL>

			Send with image/video/gif:
			1. !sticker
			2. !sticker !crop
		`)
	case "USER":
		message = strings.TrimSpace(`
			LIST COMMAND
			1. !token
		`)
	case "ADMIN":
		message = strings.TrimSpace(`
			LIST COMMAND
			1. !listmapel
			2. !pdf <nomor dari !listmapel>
			3. !pdf <nama mapel>
			4. !answer <nomor dari !listmapel> -<jawaban>
			5. !answer <nama mapel> -<jawaban>
		`)
	case "OWNER":
		message = strings.TrimSpace(`
			COMMANDS LIST

			# ADMIN
			1. !listmapel
			2. !pdf <nomor dari !listmapel>
			3. !pdf <nama mapel>
			4. !answer <nomor dari !listmapel> -<jawaban>
			5. !answer <nama mapel> -<jawaban>

			# USER
			1. !token

			# COMMON
			From Url:
			1. !sticker <video/gif/image URL>
			2. !sticker !crop <video/gif/image URL>

			Send with image/video/gif:
			1. !sticker
			2. !sticker !crop
	`)
	}

	lines := strings.Split(message, "\n")
	for i := range lines {
		lines[i] = strings.TrimLeft(lines[i], "\t ")
	}
	message = strings.Join(lines, "\n")

	ctx.Reply(message)
}