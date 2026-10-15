package call

import (
	"bytes"
	"os/exec"
	"testing"
	"time"
)

func requireFFmpeg(t *testing.T) {
	t.Helper()
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		t.Skip("ffmpeg not installed")
	}
	if args, _ := h264EncoderArgs(); args == nil {
		t.Skip("ffmpeg has no h264 encoder")
	}
}

// testJPEGs renders n distinct 320x240 JPEG pictures with ffmpeg.
func testJPEGs(t *testing.T, n int) [][]byte {
	t.Helper()
	out, err := exec.Command("ffmpeg", "-hide_banner", "-loglevel", "error",
		"-f", "lavfi", "-i", "testsrc=size=320x240:rate=15", "-frames:v", itoa(n),
		"-q:v", "5", "-f", "mjpeg", "pipe:1").Output()
	if err != nil {
		t.Fatalf("generate jpegs: %v", err)
	}
	var pics [][]byte
	rest := out
	for {
		pic, r, ok := splitJPEG(rest)
		if !ok {
			break
		}
		pics = append(pics, append([]byte(nil), pic...))
		rest = r
	}
	if len(pics) != n {
		t.Fatalf("generated %d pictures, want %d", len(pics), n)
	}
	return pics
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	s := ""
	for n > 0 {
		s = string(rune('0'+n%10)) + s
		n /= 10
	}
	return s
}

func TestSplitJPEG(t *testing.T) {
	a := []byte{0xff, 0xd8, 1, 2, 0xff, 0x00, 3, 0xff, 0xd9}
	b := []byte{0xff, 0xd8, 9, 0xff, 0xd9}
	stream := append(append([]byte{}, a...), b...)
	pic, rest, ok := splitJPEG(stream[:len(stream)-1])
	if !ok || !bytes.Equal(pic, a) {
		t.Fatalf("first picture = %v ok=%v", pic, ok)
	}
	if _, _, ok := splitJPEG(rest); ok {
		t.Fatal("incomplete second picture must not be returned")
	}
	pic, rest, ok = splitJPEG(stream)
	if !ok || !bytes.Equal(pic, a) || !bytes.Equal(rest, b) {
		t.Fatal("split of two pictures wrong")
	}
}

func TestStripAndSplitAUD(t *testing.T) {
	au1 := []byte{0, 0, 0, 1, 0x09, 0xf0, 0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x65, 3}
	au2 := []byte{0, 0, 1, 0x09, 0xf0, 0, 0, 1, 0x41, 4}
	stream := append(append([]byte{}, au1...), au2...)
	i := nextAUD(stream, 4)
	if i != len(au1) {
		t.Fatalf("second AUD at %d, want %d", i, len(au1))
	}
	got := stripAUD(stream[:i])
	want := []byte{0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x65, 3}
	if !bytes.Equal(got, want) {
		t.Fatalf("stripAUD = %v want %v", got, want)
	}
}

// TestTranscodeRoundTrip pushes pictures through the real ffmpeg encoder and
// decoder and checks that pictures come out, with low latency.
func TestTranscodeRoundTrip(t *testing.T) {
	requireFFmpeg(t)
	pics := testJPEGs(t, 30)

	type unit struct {
		au []byte
		d  time.Duration
	}
	aus := make(chan unit, 64)
	enc, err := NewJPEGToH264(func(au []byte, d time.Duration) { aus <- unit{au, d} })
	if err != nil {
		t.Fatal(err)
	}
	defer enc.Close()
	dec := NewH264ToJPEG()
	defer dec.Close()

	start := time.Now()
	var firstAU time.Duration
	gotAU, gotJPEG := 0, 0
	sent := 0
	tick := time.NewTicker(66 * time.Millisecond)
	defer tick.Stop()
	deadline := time.After(15 * time.Second)
	for gotJPEG < 20 {
		select {
		case <-tick.C:
			if sent < len(pics) {
				enc.WriteJPEG(pics[sent])
				sent++
			}
		case u := <-aus:
			if gotAU == 0 {
				firstAU = time.Since(start)
				if !bytes.HasPrefix(u.au, []byte{0, 0, 0, 1}) && !bytes.HasPrefix(u.au, []byte{0, 0, 1}) {
					t.Fatalf("access unit is not Annex-B: % x", u.au[:8])
				}
				if nextAUD(u.au, 0) == 0 {
					t.Fatal("access unit still starts with an AUD")
				}
			}
			gotAU++
			dec.WriteAU(u.au)
		case pic := <-dec.Frames():
			if !bytes.HasPrefix(pic, []byte{0xff, 0xd8}) || !bytes.HasSuffix(pic, []byte{0xff, 0xd9}) {
				t.Fatal("decoder output is not a JPEG")
			}
			gotJPEG++
		case <-deadline:
			t.Fatalf("timeout: sent=%d accessUnits=%d jpegs=%d", sent, gotAU, gotJPEG)
		}
	}
	t.Logf("first access unit after %v, %d AUs for %d pictures, %d JPEGs decoded", firstAU, gotAU, sent, gotJPEG)
	if firstAU > 3*time.Second {
		t.Fatalf("encoder too slow to produce first access unit: %v", firstAU)
	}
}

func TestForceKeyframeRestartsEncoder(t *testing.T) {
	requireFFmpeg(t)
	pics := testJPEGs(t, 10)
	aus := make(chan []byte, 64)
	enc, err := NewJPEGToH264(func(au []byte, _ time.Duration) { aus <- au })
	if err != nil {
		t.Fatal(err)
	}
	defer enc.Close()
	for i := 0; i < 5; i++ {
		enc.WriteJPEG(pics[i])
		time.Sleep(66 * time.Millisecond)
	}
	enc.ForceKeyframe()
	time.Sleep(400 * time.Millisecond)
	for len(aus) > 0 {
		<-aus
	}
	deadline := time.After(10 * time.Second)
	for i := 5; ; i = (i + 1) % len(pics) {
		enc.WriteJPEG(pics[i])
		select {
		case au := <-aus:
			if !containsNAL(au, 5) {
				t.Fatalf("first unit after restart is not an IDR")
			}
			return
		case <-time.After(100 * time.Millisecond):
		case <-deadline:
			t.Fatal("no access unit after restart")
		}
	}
}

func containsNAL(au []byte, typ byte) bool { return hasNAL(au, typ) }
