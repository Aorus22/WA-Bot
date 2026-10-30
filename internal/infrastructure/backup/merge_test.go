package backup

import (
	"context"
	"fmt"
	"testing"

	"wa-bot/internal/infrastructure/objectstorage"
)

func TestMergeAddsOnlyWhatIsMissing(t *testing.T) {
	f := newFixture(t)
	ctx := context.Background()
	rep := f.backup(t)

	// Live moves on: some old messages are gone, one was edited, a new one arrived,
	// a chat was lost, one media object disappeared.
	db := openDB(t, f.messages)
	defer db.Close()
	mustExec(t, db, `DELETE FROM messages WHERE id IN ('m0000', 'm0001', 'm0299')`)
	mustExec(t, db, `UPDATE messages SET content = 'edited live' WHERE id = 'm0100'`)
	mustExec(t, db, `INSERT INTO messages (id, chat_id, content, timestamp) VALUES ('fresh', 'chat0@s.whatsapp.net', 'new', 99999999999999)`)
	mustExec(t, db, `DELETE FROM chats WHERE id = 'chat2@s.whatsapp.net'`) // cascades its messages
	ref := f.mediaRef("img/a.jpg")
	if err := f.media.Delete(ctx, ref); err != nil {
		t.Fatal(err)
	}
	liveBefore := queryString(t, db, `SELECT count(*) FROM messages`)

	merged, err := Merge(ctx, f.target, testAccount, rep.ID, MergeTarget{MessagesDB: f.messages, Media: f.media}, Options{WorkDir: f.dir})
	if err != nil {
		t.Fatal(err)
	}
	if !(len(merged.Lost) == 0) {
		t.Fatalf("lost: %+v", merged.Lost)
	}
	// Everything the backup had is back, nothing is doubled, live changes stay.
	if got := queryString(t, db, `SELECT count(*) FROM messages WHERE id = 'm0000'`); got != "1" {
		t.Errorf("deleted message not restored: %s", got)
	}
	if got := queryString(t, db, `SELECT content FROM messages WHERE id = 'm0100'`); got != "edited live" {
		t.Errorf("live edit overwritten: %q", got)
	}
	if got := queryString(t, db, `SELECT count(*) FROM messages WHERE id = 'fresh'`); got != "1" {
		t.Errorf("live-only message lost: %s", got)
	}
	if got := queryString(t, db, `SELECT count(*) FROM chats`); got != "3" {
		t.Errorf("chats = %s", got)
	}
	if got := queryString(t, db, `SELECT count(*) FROM messages`); got != fmt.Sprint(302+1) {
		t.Errorf("messages = %s (was %s)", got, liveBefore)
	}
	if merged.Added["messages"] == 0 || merged.Present["messages"] == 0 {
		t.Errorf("counts: added %v present %v", merged.Added, merged.Present)
	}
	if ok, _ := f.media.Exists(ctx, ref); !ok || merged.MediaAdded != 1 || merged.MediaPresent != 3 {
		t.Errorf("media: exists %v, added %d, present %d", ok, merged.MediaAdded, merged.MediaPresent)
	}

	// A second merge finds nothing to add.
	again, err := Merge(ctx, f.target, testAccount, rep.ID, MergeTarget{MessagesDB: f.messages, Media: f.media}, Options{WorkDir: f.dir})
	if err != nil {
		t.Fatal(err)
	}
	for table, n := range again.Added {
		if n != 0 {
			t.Errorf("second merge added %d to %s", n, table)
		}
	}
	if again.MediaAdded != 0 {
		t.Errorf("second merge added %d media", again.MediaAdded)
	}
}

func (f *fixture) mediaRef(key string) objectstorage.ObjectRef {
	return objectstorage.ObjectRef{AccountID: testAccount, Bucket: objectstorage.DefaultBucket, Key: key}
}
