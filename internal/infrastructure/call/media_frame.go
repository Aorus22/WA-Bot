package call

import (
	"encoding/binary"
	"errors"
	"math"
)

// Binary media frame kinds (PRD §17). 0x01/0x02 travel client→server (the
// browser encodes/sends media); 0x03/0x04 travel server→client (incoming media
// forwarded to the browser).
const (
	FrameKindOutgoingAudio = 0x01
	FrameKindOutgoingVideo = 0x02
	FrameKindIncomingAudio = 0x03
	FrameKindIncomingVideo = 0x04
	// FrameKindKeyframe is a small control frame sent to the browser when the
	// call requests an IDR keyframe (meowcaller PLI/FIR). It is outside the
	// 0x01-0x04 media range and carries an empty metadata/payload.
	FrameKindKeyframe = 0x05
)

var (
	// ErrFrameTooShort indicates the frame is smaller than the 3-byte header.
	ErrFrameTooShort = errors.New("media frame too short")
	// ErrFrameBadMetadata indicates the frame's declared metadata length exceeds
	// the available payload.
	ErrFrameBadMetadata = errors.New("media frame bad metadata length")
	// ErrFrameMetadataTooLong indicates the frame's metadata exceeds the uint16
	// length field and cannot be encoded.
	ErrFrameMetadataTooLong = errors.New("media frame metadata too long")
)

// EncodeFrame builds a binary media frame:
//   - byte 0: media kind
//   - bytes 1-2: metadata JSON length (big-endian uint16)
//   - next N bytes: metadata JSON
//   - remaining: payload
//
// It returns an error if the metadata exceeds 65535 bytes (the uint16 length
// field cannot represent it).
func EncodeFrame(kind byte, metadata []byte, payload []byte) ([]byte, error) {
	if len(metadata) > math.MaxUint16 {
		return nil, ErrFrameMetadataTooLong
	}
	metaLen := len(metadata)
	data := make([]byte, 3+metaLen+len(payload))
	data[0] = kind
	binary.BigEndian.PutUint16(data[1:3], uint16(metaLen))
	copy(data[3:], metadata)
	copy(data[3+metaLen:], payload)
	return data, nil
}

// ParseFrame decodes a binary media frame produced by EncodeFrame.
func ParseFrame(data []byte) (kind byte, metadata []byte, payload []byte, err error) {
	if len(data) < 3 {
		return 0, nil, nil, ErrFrameTooShort
	}
	kind = data[0]
	metaLen := int(binary.BigEndian.Uint16(data[1:3]))
	if len(data) < 3+metaLen {
		return 0, nil, nil, ErrFrameBadMetadata
	}
	metadata = data[3 : 3+metaLen]
	payload = data[3+metaLen:]
	return kind, metadata, payload, nil
}
