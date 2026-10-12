package handlers

import "testing"

func TestNameResolverModes(t *testing.T) {
	h := &Handler{}
	profile := &nameResolver{h: h, mode: nameModeProfile, own: map[string]bool{}, cache: map[string]string{}}
	saved := &nameResolver{h: h, mode: nameModeSaved, own: map[string]bool{}, cache: map[string]string{}}

	// Without an address-book entry, WhatsApp shows the number; "profile" keeps
	// the name the person picked.
	if got := profile.resolve("628111@s.whatsapp.net", "Budi"); got != "Budi" {
		t.Fatalf("profile mode = %q, want Budi", got)
	}
	if got := saved.resolve("628111@s.whatsapp.net", "Budi"); got != "+628111" {
		t.Fatalf("saved mode = %q, want +628111", got)
	}
	// Without a phone number there is nothing better than the profile name.
	if got := saved.resolve("2426@lid", "Budi"); got != "Budi" {
		t.Fatalf("saved mode for an unmapped LID = %q, want Budi", got)
	}
	// Groups and bots are left alone.
	if got := saved.resolve("123@g.us", "Team"); got != "Team" {
		t.Fatalf("group = %q", got)
	}
	// A stored "name" that is only the number never beats a real number.
	if got := profile.resolve("628111@s.whatsapp.net", "628111"); got != "+628111" {
		t.Fatalf("number-only stored name = %q, want +628111", got)
	}
	// The account owner is never renamed.
	own := &nameResolver{h: h, mode: nameModeSaved, own: map[string]bool{"628999@s.whatsapp.net": true}, cache: map[string]string{}}
	if got := own.resolve("628999@s.whatsapp.net", "Me"); got != "Me" {
		t.Fatalf("own = %q", got)
	}
}
