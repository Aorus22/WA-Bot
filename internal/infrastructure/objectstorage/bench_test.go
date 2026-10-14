package objectstorage

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"sync/atomic"
	"testing"
	"time"
)

// patternReader produces n bytes of cheap, non-trivial data without holding
// them in memory.
type patternReader struct{ left int64 }

func (p *patternReader) Read(b []byte) (int, error) {
	if p.left <= 0 {
		return 0, io.EOF
	}
	if int64(len(b)) > p.left {
		b = b[:p.left]
	}
	for i := range b {
		b[i] = byte(i*131 + int(p.left))
	}
	p.left -= int64(len(b))
	return len(b), nil
}

// TestStreamingMemoryAndThroughput measures what the plan asks to be measured:
// peak memory while a large object is uploaded, copied and downloaded, and the
// throughput. It is slow, so it only runs on request:
//
//	OBJECTSTORAGE_BENCH=1 go test ./internal/infrastructure/objectstorage -run Throughput -v -timeout 30m
func TestStreamingMemoryAndThroughput(t *testing.T) {
	if os.Getenv("OBJECTSTORAGE_BENCH") == "" {
		t.Skip("set OBJECTSTORAGE_BENCH=1 to run")
	}
	for _, mib := range []int64{1, 100, 1024} {
		size := mib << 20
		path := filepath.Join(t.TempDir(), "bench.sqlite")
		s, err := OpenSQLite(SQLiteOptions{Path: path})
		if err != nil {
			t.Fatal(err)
		}
		ctx := context.Background()
		ref := ObjectRef{AccountID: "628111", Key: "bench/object.bin"}

		runtime.GC()
		var peak atomic.Uint64
		stop := make(chan struct{})
		done := make(chan struct{})
		go func() {
			defer close(done)
			var m runtime.MemStats
			for {
				runtime.ReadMemStats(&m)
				if m.HeapInuse > peak.Load() {
					peak.Store(m.HeapInuse)
				}
				select {
				case <-stop:
					return
				case <-time.After(20 * time.Millisecond):
				}
			}
		}()

		t0 := time.Now()
		if _, err := s.Put(ctx, ref, &patternReader{left: size}, PutOptions{}); err != nil {
			t.Fatal(err)
		}
		put := time.Since(t0)
		t0 = time.Now()
		if err := s.Copy(ctx, ref, ObjectRef{AccountID: "628111", Key: "bench/copy.bin"}); err != nil {
			t.Fatal(err)
		}
		cp := time.Since(t0)
		t0 = time.Now()
		rc, _, err := s.Open(ctx, ref, GetOptions{})
		if err != nil {
			t.Fatal(err)
		}
		n, err := io.Copy(io.Discard, rc)
		rc.Close()
		if err != nil || n != size {
			t.Fatalf("read %d of %d: %v", n, size, err)
		}
		get := time.Since(t0)
		close(stop)
		<-done

		st, _ := os.Stat(path)
		mbps := func(d time.Duration) float64 { return float64(size) / (1 << 20) / d.Seconds() }
		t.Logf("%5d MiB: put %6.0f MiB/s, copy %6.0f MiB/s, get %6.0f MiB/s | peak heap %4d MiB | file %d MiB",
			mib, mbps(put), mbps(cp), mbps(get), peak.Load()>>20, st.Size()>>20)
		s.Close()
	}
}
