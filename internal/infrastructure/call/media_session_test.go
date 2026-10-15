package call

import (
	"errors"
	"io"
	"testing"

	"github.com/purpshell/meowcaller"
)

func newTestService(t *testing.T, id string) *CallService {
	t.Helper()
	s := NewCallService(nil, nil, nil, nil)
	s.active = &CallSession{ID: id}
	return s
}

func TestMediaSessionTokenIsSingleUse(t *testing.T) {
	s := newTestService(t, "call_1")
	sess, err := s.CreateMediaSession("call_1")
	if err != nil {
		t.Fatal(err)
	}
	if err := s.ValidateMediaSession("call_1", "wrong"); !errors.Is(err, ErrMediaSessionInvalid) {
		t.Fatalf("wrong token: %v", err)
	}
	if err := s.ValidateMediaSession("call_1", sess.Token); err != nil {
		t.Fatalf("valid token rejected: %v", err)
	}
	if err := s.ValidateMediaSession("call_1", sess.Token); !errors.Is(err, ErrMediaSessionInvalid) {
		t.Fatalf("token reused: %v", err)
	}
	again, err := s.CreateMediaSession("call_1")
	if err != nil || again.Token == sess.Token {
		t.Fatalf("a fresh token is issued after use: %v", err)
	}
	if _, err := s.CreateMediaSession("other"); !errors.Is(err, ErrCallNotActive) {
		t.Fatalf("token for inactive call: %v", err)
	}
}

func TestChannelAudioSourceBoundedAndNonBlocking(t *testing.T) {
	src := NewChannelAudioSource(3)
	frame := func(v float32) []float32 {
		f := make([]float32, meowcaller.FrameSamples)
		f[0] = v
		return f
	}
	got, err := src.ReadFrame()
	if err != nil || len(got) != meowcaller.FrameSamples || got[0] != 0 {
		t.Fatalf("empty queue must give a silence frame, got %v err %v", got[:1], err)
	}
	for i := 1; i <= 5; i++ {
		src.Feed(frame(float32(i)))
	}
	// capacity 3: the two oldest were dropped
	for _, want := range []float32{3, 4, 5} {
		got, _ := src.ReadFrame()
		if got[0] != want {
			t.Fatalf("got frame %v want %v", got[0], want)
		}
	}
	src.Close()
	if _, err := src.ReadFrame(); err != io.EOF {
		t.Fatalf("closed source: %v", err)
	}
	if src.Feed(frame(1)) {
		t.Fatal("Feed on a closed source must report false")
	}
}

func TestChannelAudioSinkDropsOldestAndCopies(t *testing.T) {
	sink := NewChannelAudioSink(2)
	buf := []float32{1}
	_ = sink.WriteFrame(buf)
	buf[0] = 2 // meowcaller reuses its buffer
	_ = sink.WriteFrame(buf)
	buf[0] = 3
	_ = sink.WriteFrame(buf)
	a, b := <-sink.Frames(), <-sink.Frames()
	if a[0] != 2 || b[0] != 3 {
		t.Fatalf("got %v %v, want the two newest frames 2 and 3", a, b)
	}
}
