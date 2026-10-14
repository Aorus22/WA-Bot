package repository

import "context"

// MediaSizer is a media storage that can say how big it is: the size of the
// SQLite database holding the media. ok is false when it is not one (S3).
type MediaSizer interface {
	MediaSize(ctx context.Context) (size int64, ok bool, err error)
}

// DatabaseSize is the size of the message database in bytes: the pages SQLite
// has allocated, free ones included, as the file occupies them on disk.
func (s *MessageStore) DatabaseSize() (int64, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var pages, pageSize int64
	if err := s.db.QueryRow("PRAGMA page_count").Scan(&pages); err != nil {
		return 0, err
	}
	if err := s.db.QueryRow("PRAGMA page_size").Scan(&pageSize); err != nil {
		return 0, err
	}
	return pages * pageSize, nil
}
