package main

import (
    "database/sql"
    "fmt"
    "os"
    "time"

    "github.com/robfig/cron/v3"
    _ "github.com/mattn/go-sqlite3"
)

func clearChatHistory() error {
    waClient.Disconnect()
    fmt.Println("Client disconnected for cron job")

    db, err := sql.Open("sqlite3", dbUrl)
    if err != nil {
        return fmt.Errorf("failed to open database: %v", err)
    }
    defer db.Close()

    tx, err := db.Begin()
    if err != nil {
        return fmt.Errorf("failed to begin transaction: %v", err)
    }

    query1 := "DELETE FROM whatsmeow_message_secrets"
    _, err = tx.Exec(query1)
    if err != nil {
        tx.Rollback()
        return fmt.Errorf("failed to delete message secrets: %v", err)
    }

    query2 := "DELETE FROM whatsmeow_app_state_mutation_macs"
    _, err = tx.Exec(query2)
    if err != nil {
        tx.Rollback()
        return fmt.Errorf("failed to delete app state mutation macs: %v", err)
    }

    err = tx.Commit()
    if err != nil {
        return fmt.Errorf("failed to commit transaction: %v", err)
    }

    err = waClient.Connect()
    if err != nil {
        return fmt.Errorf("failed to reconnect client: %v", err)
    }

    fmt.Printf("Message history and app state mutation macs successfully cleared and client reconnected at %s\n", time.Now().Format(time.RFC1123))
    return nil
}

func setupCron() {
    schedule := os.Getenv("CRON_SCHEDULE")
    if schedule == "" {
        panic("CRON_SCHEDULE environment variable is not set")
    }

    c := cron.New()

    _, err := c.AddFunc(schedule, func() {
        err := clearChatHistory()
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
