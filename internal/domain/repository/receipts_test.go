package repository

import "testing"

func TestGroupReceiptsTrackDeliveryAndRead(t *testing.T) {
	s := newTestStore(t)
	const group = "999@g.us"
	if _, err := s.db.Exec(`INSERT INTO chats (id, name, is_group) VALUES (?, 'g', 1)`, group); err != nil {
		t.Fatal(err)
	}
	if err := s.SaveMessage(&Message{ID: "m1", ChatID: group, From: "me", To: group, Content: "hi", Timestamp: 1, Status: "sent", Type: "text"}); err != nil {
		t.Fatal(err)
	}

	own := map[string]bool{"1": true}
	s.SaveReceipt("m1", "2@s.whatsapp.net", ReceiptDelivered, 100)
	s.SaveReceipt("m1", "3@s.whatsapp.net", ReceiptRead, 200) // read with no delivery event
	s.SaveReceipt("m1", "2@s.whatsapp.net", ReceiptRead, 300)
	s.SaveReceipt("m1", "2@s.whatsapp.net", ReceiptDelivered, 999) // replay is ignored

	delivered, read, err := s.CountReceipts(group, "m1", own)
	if err != nil || delivered != 2 || read != 2 {
		t.Fatalf("delivered=%d read=%d err=%v", delivered, read, err)
	}
	got, _ := s.GetMessageReceipts(group, "m1")
	for _, r := range got {
		if r.JID == "2@s.whatsapp.net" && (r.DeliveredAt != 100 || r.ReadAt != 300) {
			t.Fatalf("member 2 = %+v", r)
		}
		if r.JID == "3@s.whatsapp.net" && (r.DeliveredAt != 200 || r.ReadAt != 200) {
			t.Fatalf("member 3 = %+v", r)
		}
	}

	if ok, _ := s.UpgradeMessageStatus("m1", "read"); !ok {
		t.Fatal("expected upgrade to read")
	}
	if ok, _ := s.UpgradeMessageStatus("m1", "delivered"); ok {
		t.Fatal("status must not move backwards")
	}
}

func TestPrivateTickFollowsReceiptsInAnyOrder(t *testing.T) {
	s := newTestStore(t)
	const chat = "2@s.whatsapp.net"
	if err := s.SaveMessage(&Message{ID: "p1", ChatID: chat, From: "me", To: chat, Content: "hi", Timestamp: 1, Status: "sent", Type: "text"}); err != nil {
		t.Fatal(err)
	}

	if status, changed, err := s.RecordReceipt("p1", chat, ReceiptRead, 300, nil, 1); err != nil || !changed || status != "read" {
		t.Fatalf("read: status=%q changed=%v err=%v", status, changed, err)
	}
	// A delivery receipt arriving after the read one must not undo it.
	if status, changed, err := s.RecordReceipt("p1", chat, ReceiptDelivered, 400, nil, 1); err != nil || changed || status != "read" {
		t.Fatalf("late delivered: status=%q changed=%v err=%v", status, changed, err)
	}
	var stored string
	if err := s.db.QueryRow(`SELECT status FROM messages WHERE id = 'p1'`).Scan(&stored); err != nil || stored != "read" {
		t.Fatalf("stored status = %q err=%v", stored, err)
	}
	// Receipts for someone else's message are ignored.
	if _, changed, _ := s.RecordReceipt("nope", chat, ReceiptRead, 1, nil, 1); changed {
		t.Fatal("unknown message changed")
	}
}
