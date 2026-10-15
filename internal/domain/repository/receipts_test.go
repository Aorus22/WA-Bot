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
