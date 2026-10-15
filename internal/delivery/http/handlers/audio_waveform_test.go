package handlers

import (
	"os/exec"
	"testing"
)

func TestWaveformFromPCM(t *testing.T) {
	if waveformFromPCM(nil) != nil {
		t.Fatal("no samples, no waveform")
	}
	if wf := waveformFromPCM(make([]int16, 1000)); len(wf) != waveformBars || wf[0] != 0 {
		t.Fatalf("silence = %v", wf)
	}

	// First half loud, second half quiet.
	samples := make([]int16, 6400)
	for i := range samples {
		if i < 3200 {
			samples[i] = 20000
		} else {
			samples[i] = 1250
		}
	}
	wf := waveformFromPCM(samples)
	if len(wf) != waveformBars {
		t.Fatalf("bars = %d", len(wf))
	}
	if wf[0] != 100 || wf[waveformBars-1] >= wf[0] || wf[waveformBars-1] == 0 {
		t.Fatalf("loud=%d quiet=%d", wf[0], wf[waveformBars-1])
	}
	for _, b := range wf {
		if b > 100 {
			t.Fatalf("bar out of range: %d", b)
		}
	}
}

func TestAudioWaveformDecodesRealAudio(t *testing.T) {
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		t.Skip("ffmpeg not installed")
	}
	// 2s tone fading out, as webm/opus like a browser recording.
	out, err := exec.Command("ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=2",
		"-af", "afade=t=out:st=0:d=2", "-c:a", "libopus", "-f", "webm", "pipe:1").Output()
	if err != nil {
		t.Skipf("cannot synthesize audio: %v", err)
	}
	wf, secs, err := audioWaveform(out)
	if err != nil || secs != 2 || len(wf) != waveformBars {
		t.Fatalf("waveform len=%d secs=%d err=%v", len(wf), secs, err)
	}
	if wf[2] <= wf[waveformBars-3] {
		t.Fatalf("fade-out not reflected: start=%d end=%d", wf[2], wf[waveformBars-3])
	}
}
