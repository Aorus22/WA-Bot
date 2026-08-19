package call

import (
	"context"
	"os"

	edge_tts "github.com/bytectlgo/edge-tts/pkg/edge_tts"
)

// EdgeTTSProvider synthesizes speech via Microsoft Edge TTS, writing MP3 bytes to
// a temporary file. The provider is only meaningful when CALL_TTS_PROVIDER=edge.
type EdgeTTSProvider struct {
	defaultVoice string
}

// NewEdgeTTSProvider builds an Edge TTS provider with the given default voice.
// An empty voice falls back to id-ID-GadisNeural.
func NewEdgeTTSProvider(defaultVoice string) *EdgeTTSProvider {
	if defaultVoice == "" {
		defaultVoice = "id-ID-GadisNeural"
	}
	return &EdgeTTSProvider{defaultVoice: defaultVoice}
}

// DefaultVoice returns the configured default voice.
func (p *EdgeTTSProvider) DefaultVoice() string {
	return p.defaultVoice
}

// Synthesize renders req.Text into an MP3 temp file and returns an AudioResult
// whose Cleanup removes the file once playback has finished. The voice falls
// back to the provider default when req.Voice is empty.
func (p *EdgeTTSProvider) Synthesize(ctx context.Context, req TTSRequest) (*AudioResult, error) {
	voice := req.Voice
	if voice == "" {
		voice = p.defaultVoice
	}

	// Create a temp file to hold the MP3 output. Save() recreates/truncates it,
	// so we only need the path here.
	tmp, err := os.CreateTemp("", "wa-tts-*.mp3")
	if err != nil {
		return nil, err
	}
	path := tmp.Name()
	if err := tmp.Close(); err != nil {
		_ = os.Remove(path)
		return nil, err
	}

	comm := edge_tts.NewCommunicate(req.Text, voice)
	if err := comm.Save(ctx, path, ""); err != nil {
		_ = os.Remove(path)
		return nil, err
	}

	return &AudioResult{
		Path:   path,
		Format: "mp3",
		Cleanup: func() {
			_ = os.Remove(path)
		},
	}, nil
}
