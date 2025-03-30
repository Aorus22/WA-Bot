package utils

import (
    "database/sql"
    "fmt"
    "os"
    "time"

    "github.com/robfig/cron/v3"
    _ "github.com/mattn/go-sqlite3"
    "go.mau.fi/whatsmeow"
)

func clearChatHistory(dbPath string, client *whatsmeow.Client) error {
    client.Disconnect()
    fmt.Println("Client disconnected for cron job")

    db, err := sql.Open("sqlite3", dbPath)
    if err != nil {
        return fmt.Errorf("failed to open database: %v", err)
    }
    defer db.Close()

    query := "DELETE FROM whatsmeow_message_secrets"
    _, err = db.Exec(query)
    if err != nil {
        return fmt.Errorf("failed to delete message history: %v", err)
    }

    err = client.Connect()
    if err != nil {
        return fmt.Errorf("failed to reconnect client: %v", err)
    }

    fmt.Printf("Message history successfully cleared and client reconnected at %s\n", time.Now().Format(time.RFC1123))
    return nil
}

func SetupCron(dbPath string, client *whatsmeow.Client) {
    schedule := os.Getenv("CRON_SCHEDULE")
    if schedule == "" {
        panic("CRON_SCHEDULE environment variable is not set")
    }

    c := cron.New()

    _, err := c.AddFunc(schedule, func() {
        err := clearChatHistory(dbPath, client)
        if err != nil {
            fmt.Printf("Error while clearing messages via cron: %v\n", err)
        }
    })
    if err != nil {
        panic(fmt.Sprintf("Failed to add cron job: %v", err))
    }

    c.Start()
    fmt.Printf("Cron job set up to clear messages on schedule: %s\n", schedule)
}
