package main

import "testing"

func TestOverallPercentMovesForwardAcrossStagesAndAccounts(t *testing.T) {
	last := -1
	for _, step := range []struct {
		stage       string
		done, total int64
	}{{"snapshot", 0, 0}, {"chat", 1, 4}, {"chat", 4, 4}, {"media", 0, 10}, {"media", 5, 10}, {"media", 10, 10}, {"manifest", 0, 0}} {
		got := overallPercent(1, 2, step.stage, step.done, step.total)
		if got < last {
			t.Fatalf("%s %d/%d went back: %d after %d", step.stage, step.done, step.total, got, last)
		}
		last = got
	}
	if last >= 50 {
		t.Fatalf("the first of two accounts must stay under half, got %d", last)
	}
	if got := overallPercent(2, 2, "manifest", 0, 0); got < 95 || got > 99 {
		t.Fatalf("the last stage of the last account is %d", got)
	}
	if got := overallPercent(1, 1, "media", 10, 10); got != 95 {
		t.Fatalf("media done on a single account should read 95, got %d", got)
	}
}
