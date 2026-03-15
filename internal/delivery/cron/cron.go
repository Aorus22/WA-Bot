package cron

import (
	"context"
	"fmt"
	"sync"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/lua"

	"github.com/robfig/cron/v3"
)

type CronScheduler struct {
	cronRepo   repository.CronJobRepository
	luaService *lua.LuaService
	cron       *cron.Cron
	entryIDs   map[string]cron.EntryID
	mu         sync.Mutex
}

func NewCronScheduler(cronRepo repository.CronJobRepository, luaService *lua.LuaService) *CronScheduler {
	// Custom parser: seconds optional, support standard 5-field and 6-field
	parser := cron.NewParser(
		cron.SecondOptional | cron.Minute | cron.Hour | cron.Dom | cron.Month | cron.Dow | cron.Descriptor,
	)
	return &CronScheduler{
		cronRepo:   cronRepo,
		luaService: luaService,
		cron:       cron.New(cron.WithParser(parser)),
		entryIDs:   make(map[string]cron.EntryID),
	}
}

func (c *CronScheduler) Start() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	// Initialize cron if stopped (optional safety)
	if c.cron == nil {
		parser := cron.NewParser(
			cron.SecondOptional | cron.Minute | cron.Hour | cron.Dom | cron.Month | cron.Dow | cron.Descriptor,
		)
		c.cron = cron.New(cron.WithParser(parser))
	}

	err := c.loadJobs()
	if err != nil {
		return err
	}

	c.cron.Start()
	fmt.Println("Cron scheduler started")
	return nil
}

func (c *CronScheduler) Stop() {
	if c.cron != nil {
		c.cron.Stop()
	}
}

func (c *CronScheduler) Reload() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	// Stop current cron and clear entries
	if c.cron != nil {
		c.cron.Stop()
	}
	parser := cron.NewParser(
		cron.SecondOptional | cron.Minute | cron.Hour | cron.Dom | cron.Month | cron.Dow | cron.Descriptor,
	)
	c.cron = cron.New(cron.WithParser(parser))
	c.entryIDs = make(map[string]cron.EntryID)

	err := c.loadJobs()
	if err != nil {
		return err
	}

	c.cron.Start()
	fmt.Println("Cron scheduler reloaded")
	return nil
}

func (c *CronScheduler) loadJobs() error {
	jobs, err := c.cronRepo.GetAllCron(context.Background())
	if err != nil {
		return fmt.Errorf("failed to fetch cron jobs: %v", err)
	}

	for _, job := range jobs {
		if !job.IsActive {
			continue
		}

		jobCopy := job // Create local copy for closure
		entryID, err := c.cron.AddFunc(job.Schedule, func() {
			fmt.Printf("[CRON] Running job: %s\n", jobCopy.Name)
			c.luaService.RunCronScript(context.Background(), jobCopy.Script)
		})

		if err != nil {
			fmt.Printf("[CRON] Failed to schedule job %s: %v\n", job.Name, err)
			continue
		}

		c.entryIDs[job.ID] = entryID
		fmt.Printf("[CRON] Scheduled job: %s (%s)\n", job.Name, job.Schedule)
	}

	return nil
}
