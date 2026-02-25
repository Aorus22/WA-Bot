package main

import (
	"fmt"

	"wa-bot/internal/config"
)

func main() {
	app, err := config.InitializeApp()
	if err != nil {
		panic(err)
	}

	if err := app.Run(); err != nil {
		panic(err)
	}

	fmt.Println("Application stopped gracefully")
}
