package call

import "context"

// TTSRequest is the input to a TTS synthesis.
type TTSRequest struct {
	Text  string
	Voice string
	// ReferenceID optionally selects a specific voice library / voice ID for
	// providers that support it (e.g. Fish Audio). Empty falls back to the
	// provider's default voice.
	ReferenceID string
}

// AudioResult describes a playable audio source resolved for a media-mode call.
type AudioResult struct {
	// Path is the path to a temporary audio file (mp3/wav) on disk.
	Path string
	// Format is "mp3" or "wav".
	Format string
	// Cleanup removes the temp file after playback (may be nil).
	Cleanup func()
}

// VideoResult describes a playable video source resolved for a media-mode call.
type VideoResult struct {
	Path   string
	Format string // "mp4" etc, sniffed container
	Cleanup func()
}

// TTSProvider synthesizes text into a playable audio file.
type TTSProvider interface {
	Synthesize(ctx context.Context, req TTSRequest) (*AudioResult, error)
	DefaultVoice() string
}
