package objectstorage

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"testing"
)

// runStoreContract holds every provider to the same behaviour: it is what makes
// the SQLite engine and the remote S3 adapter interchangeable.
func runStoreContract(t *testing.T, s Store) {
	ctx := context.Background()
	acct := "628111"
	r := func(key string) ObjectRef { return ObjectRef{AccountID: acct, Key: key} }

	t.Run("put-open-stat", func(t *testing.T) {
		for _, size := range []int{0, 1, 4095, 3 << 20} {
			data := randomBytes(size, int64(size)+1)
			key := fmt.Sprintf("contract/size-%d.bin", size)
			info, err := s.Put(ctx, r(key), bytes.NewReader(data), PutOptions{ContentType: "application/x-contract", Metadata: map[string]string{"origin": "test"}})
			if err != nil {
				t.Fatalf("put: %v", err)
			}
			if info.Size != int64(size) {
				t.Fatalf("put info size %d", info.Size)
			}
			rc, got, err := s.Open(ctx, r(key), GetOptions{})
			if err != nil {
				t.Fatalf("open: %v", err)
			}
			body, _ := io.ReadAll(rc)
			rc.Close()
			if !bytes.Equal(body, data) || got.Size != int64(size) {
				t.Fatalf("size %d: content differs", size)
			}
			st, err := s.Stat(ctx, r(key))
			if err != nil || st.Size != int64(size) || st.ContentType != "application/x-contract" || st.Metadata["origin"] != "test" || st.LastModified.IsZero() {
				t.Fatalf("stat: %+v %v", st, err)
			}
			if st.SHA256 == "" {
				t.Fatalf("size %d: no checksum recorded", size)
			}
		}
	})

	t.Run("ranges", func(t *testing.T) {
		data := randomBytes(10000, 77)
		s.Put(ctx, r("contract/range.bin"), bytes.NewReader(data), PutOptions{})
		cases := []struct{ start, end int64 }{{0, 0}, {0, 99}, {500, 1500}, {9999, 9999}, {9000, -1}, {0, 99999}}
		for _, c := range cases {
			rc, _, err := s.Open(ctx, r("contract/range.bin"), GetOptions{Range: &ByteRange{Start: c.start, End: c.end}})
			if err != nil {
				t.Fatalf("range %d-%d: %v", c.start, c.end, err)
			}
			got, _ := io.ReadAll(rc)
			rc.Close()
			want := data[c.start:]
			if c.end >= 0 && c.end < 10000 {
				want = data[c.start : c.end+1]
			}
			if !bytes.Equal(got, want) {
				t.Errorf("range %d-%d: %d bytes, want %d", c.start, c.end, len(got), len(want))
			}
		}
		for _, bad := range []ByteRange{{10000, -1}, {-5, 3}, {50, 10}} {
			bad := bad
			if _, _, err := s.Open(ctx, r("contract/range.bin"), GetOptions{Range: &bad}); !errors.Is(err, ErrInvalidRange) {
				t.Errorf("range %+v: %v", bad, err)
			}
		}
	})

	t.Run("missing", func(t *testing.T) {
		if _, err := s.Stat(ctx, r("contract/nope")); !errors.Is(err, ErrNotFound) {
			t.Errorf("stat: %v", err)
		}
		if _, _, err := s.Open(ctx, r("contract/nope"), GetOptions{}); !errors.Is(err, ErrNotFound) {
			t.Errorf("open: %v", err)
		}
		if ok, err := s.Exists(ctx, r("contract/nope")); ok || err != nil {
			t.Errorf("exists: %v %v", ok, err)
		}
		if err := s.Delete(ctx, r("contract/nope")); err != nil {
			t.Errorf("delete of missing: %v", err)
		}
		if err := s.Copy(ctx, r("contract/nope"), r("contract/other")); !errors.Is(err, ErrNotFound) {
			t.Errorf("copy: %v", err)
		}
	})

	t.Run("copy-overwrite-delete", func(t *testing.T) {
		a, b := randomBytes(5000, 1), randomBytes(300, 2)
		s.Put(ctx, r("contract/a"), bytes.NewReader(a), PutOptions{ContentType: "image/png"})
		if err := s.Copy(ctx, r("contract/a"), r("contract/copy of a (1).png")); err != nil {
			t.Fatal(err)
		}
		rc, info, err := s.Open(ctx, r("contract/copy of a (1).png"), GetOptions{})
		if err != nil {
			t.Fatal(err)
		}
		got, _ := io.ReadAll(rc)
		rc.Close()
		if !bytes.Equal(got, a) || info.ContentType != "image/png" {
			t.Fatal("copy differs from source")
		}
		s.Put(ctx, r("contract/a"), bytes.NewReader(b), PutOptions{})
		rc, _, _ = s.Open(ctx, r("contract/a"), GetOptions{})
		got, _ = io.ReadAll(rc)
		rc.Close()
		if !bytes.Equal(got, b) {
			t.Fatal("overwrite did not replace content")
		}
		if err := s.Delete(ctx, r("contract/a")); err != nil {
			t.Fatal(err)
		}
		if ok, _ := s.Exists(ctx, r("contract/a")); ok {
			t.Fatal("still exists after delete")
		}
		if ok, _ := s.Exists(ctx, r("contract/copy of a (1).png")); !ok {
			t.Fatal("deleting the source removed the copy")
		}
	})

	t.Run("special-keys", func(t *testing.T) {
		for _, key := range []string{"history/日本語 ünï/😀 file (1).jpg", "a+b=c&d%20e?.txt", "deep/er/and/deeper/x.bin"} {
			if _, err := s.Put(ctx, r("contract/"+key), bytes.NewReader([]byte(key)), PutOptions{}); err != nil {
				t.Fatalf("put %q: %v", key, err)
			}
			rc, _, err := s.Open(ctx, r("contract/"+key), GetOptions{})
			if err != nil {
				t.Fatalf("open %q: %v", key, err)
			}
			got, _ := io.ReadAll(rc)
			rc.Close()
			if string(got) != key {
				t.Fatalf("%q: got %q", key, got)
			}
		}
	})

	t.Run("accounts-isolated", func(t *testing.T) {
		s.Put(ctx, ObjectRef{AccountID: "628111", Key: "iso/x"}, bytes.NewReader([]byte("one")), PutOptions{})
		s.Put(ctx, ObjectRef{AccountID: "628222", Key: "iso/x"}, bytes.NewReader([]byte("two")), PutOptions{})
		page, err := s.List(ctx, ObjectScope{AccountID: "628222"}, ListOptions{Prefix: "iso/"})
		if err != nil || len(page.Objects) != 1 || page.Objects[0].Ref.AccountID != "628222" {
			t.Fatalf("list: %+v %v", page, err)
		}
	})

	t.Run("list-pagination", func(t *testing.T) {
		for i := 0; i < 23; i++ {
			s.Put(ctx, r(fmt.Sprintf("page/%02d.txt", i)), bytes.NewReader([]byte("x")), PutOptions{})
		}
		var keys []string
		cursor := ""
		for n := 0; n < 10; n++ {
			page, err := s.List(ctx, ObjectScope{AccountID: acct}, ListOptions{Prefix: "page/", Limit: 10, Cursor: cursor})
			if err != nil {
				t.Fatal(err)
			}
			for _, o := range page.Objects {
				keys = append(keys, o.Ref.Key)
			}
			if page.NextCursor == "" {
				break
			}
			cursor = page.NextCursor
		}
		if len(keys) != 23 || keys[0] != "page/00.txt" || keys[22] != "page/22.txt" {
			t.Fatalf("keys: %v", keys)
		}
		empty, err := s.List(ctx, ObjectScope{AccountID: acct}, ListOptions{Prefix: "no-such-prefix/"})
		if err != nil || len(empty.Objects) != 0 {
			t.Fatalf("empty listing: %+v %v", empty, err)
		}
	})

	t.Run("invalid-keys", func(t *testing.T) {
		for _, key := range []string{"", "../x", "/abs", `a\b`} {
			if _, err := s.Put(ctx, r(key), bytes.NewReader(nil), PutOptions{}); !errors.Is(err, ErrInvalidKey) {
				t.Errorf("key %q: %v", key, err)
			}
		}
	})
}

func TestSQLiteContract(t *testing.T) {
	s, _ := newStore(t, 0)
	runStoreContract(t, s)
}
