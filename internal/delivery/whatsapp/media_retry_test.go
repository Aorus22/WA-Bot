package whatsapp

import (
	"testing"
	"time"

	"go.mau.fi/whatsmeow/types/events"
)

func TestMediaRetryHubMatchesAnswersToTheirMessage(t *testing.T) {
	hub := NewMediaRetryHub()
	first, forgetFirst := hub.Expect("m1")
	second, forgetSecond := hub.Expect("m2")
	defer forgetSecond()

	hub.Deliver(&events.MediaRetry{MessageID: "m2"})
	select {
	case got := <-second:
		if got.MessageID != "m2" {
			t.Fatalf("got %q", got.MessageID)
		}
	case <-time.After(time.Second):
		t.Fatal("the answer for m2 was not delivered")
	}
	select {
	case <-first:
		t.Fatal("the answer for m2 went to the one waiting for m1")
	default:
	}

	hub.Deliver(&events.MediaRetry{MessageID: "m1"})
	if got := <-first; got.MessageID != "m1" {
		t.Fatalf("got %q", got.MessageID)
	}
	forgetFirst()
	if hub.pending() != 1 {
		t.Fatalf("waiters left: %d", hub.pending())
	}
}

func TestMediaRetryHubDropsAnswersNobodyWaitsFor(t *testing.T) {
	hub := NewMediaRetryHub()
	hub.Deliver(&events.MediaRetry{MessageID: "nobody"}) // must not block or panic
	hub.Deliver(nil)

	answer, forget := hub.Expect("m1")
	forget()
	hub.Deliver(&events.MediaRetry{MessageID: "m1"})
	select {
	case <-answer:
		t.Fatal("delivered to a waiter that gave up")
	default:
	}
	if hub.pending() != 0 {
		t.Fatalf("waiters left: %d", hub.pending())
	}

	// Two waiters for the same message both hear the answer, once.
	a, forgetA := hub.Expect("same")
	b, forgetB := hub.Expect("same")
	defer forgetA()
	defer forgetB()
	hub.Deliver(&events.MediaRetry{MessageID: "same"})
	hub.Deliver(&events.MediaRetry{MessageID: "same"}) // a second copy does not block
	if (<-a).MessageID != "same" || (<-b).MessageID != "same" {
		t.Fatal("both waiters should hear the answer")
	}
}
