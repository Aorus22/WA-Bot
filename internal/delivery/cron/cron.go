package cron

import (
	"database/sql"
	"fmt"
	"time"

	whatsappInfra "wa-bot/internal/infrastructure/whatsapp"

	_ "github.com/mattn/go-sqlite3"
	"github.com/robfig/cron/v3"
)

type CronScheduler struct {
	client   *whatsappInfra.WhatsAppClient
	dbURL    string
	schedule string
	cron     *cron.Cron
}

func NewCronScheduler(client *whatsappInfra.WhatsAppClient, dbURL, schedule string) *CronScheduler {
	return &CronScheduler{
		client:   client,
		dbURL:    dbURL,
		schedule: schedule,
		cron:     cron.New(),
	}
}

func (c *CronScheduler) Start() error {
	_, err := c.cron.AddFunc(c.schedule, func() {
		err := c.clearChatHistory()
		if err != nil {
			fmt.Printf("Error while clearing messages via cron: %v\n", err)
		}
	})
	if err != nil {
		return fmt.Errorf("failed to add cron job: %v", err)
	}

	c.cron.Start()
	fmt.Printf("Cron job set up to clear messages on schedule: %s\n", c.schedule)
	return nil
}

func (c *CronScheduler) Stop() {
	c.cron.Stop()
}

func (c *CronScheduler) clearChatHistory() error {
	c.client.Disconnect()
	fmt.Println("Client disconnected for cron job")

	db, err := sql.Open("sqlite3", c.dbURL)
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

	err = c.client.Connect()
	if err != nil {
		return fmt.Errorf("failed to reconnect client: %v", err)
	}

	fmt.Printf("Message history and app state mutation macs successfully cleared and client reconnected at %s\n", time.Now().Format(time.RFC1123))
	return nil
}
