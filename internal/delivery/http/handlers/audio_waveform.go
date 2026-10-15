package handlers

import (
	"bytes"
	"fmt"
	"math"
	"os/exec"
)

const (
	waveformBars       = 64
	waveformSampleRate = 8000
)

// audioWaveform decodes any audio ffmpeg understands and returns the 64-bar
// waveform WhatsApp draws for voice notes (each bar 0-100) and the duration
// in whole seconds. Requires ffmpeg in PATH, like the PTT transcode.
func audioWaveform(in []byte) (waveform []byte, seconds uint32, err error) {
	cmd := exec.Command("ffmpeg", "-hide_banner", "-loglevel", "error", "-i", "pipe:0",
		"-f", "s16le", "-ac", "1", "-ar", fmt.Sprint(waveformSampleRate), "pipe:1")
	cmd.Stdin = bytes.NewReader(in)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return nil, 0, fmt.Errorf("ffmpeg: %v: %s", err, stderr.String())
	}
	raw := stdout.Bytes()
	samples := make([]int16, len(raw)/2)
	for i := range samples {
		samples[i] = int16(uint16(raw[2*i]) | uint16(raw[2*i+1])<<8)
	}
	waveform = waveformFromPCM(samples)
	seconds = uint32(math.Ceil(float64(len(samples)) / waveformSampleRate))
	return waveform, seconds, nil
}

// waveformFromPCM reduces samples to waveformBars RMS bars scaled so the loudest is 100.
func waveformFromPCM(samples []int16) []byte {
	if len(samples) == 0 {
		return nil
	}
	bars := make([]float64, waveformBars)
	var peak float64
	for i := range bars {
		from := i * len(samples) / waveformBars
		to := (i + 1) * len(samples) / waveformBars
		if to <= from {
			to = from + 1
		}
		if to > len(samples) {
			to = len(samples)
		}
		var sum float64
		for _, v := range samples[from:to] {
			f := float64(v)
			sum += f * f
		}
		bars[i] = math.Sqrt(sum / float64(to-from))
		if bars[i] > peak {
			peak = bars[i]
		}
	}
	out := make([]byte, waveformBars)
	if peak == 0 {
		return out
	}
	for i, b := range bars {
		// sqrt lifts quiet passages so speech does not look flat.
		out[i] = byte(math.Round(math.Sqrt(b/peak) * 100))
	}
	return out
}
