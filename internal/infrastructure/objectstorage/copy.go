package objectstorage

import (
	"context"
	"fmt"
	"strings"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

// AccountLister is implemented by providers that can enumerate the accounts
// they hold objects for.
type AccountLister interface {
	ListAccounts(ctx context.Context) ([]string, error)
}

func (s *SQLiteStore) ListAccounts(ctx context.Context) ([]string, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT DISTINCT account_id FROM objects ORDER BY account_id`)
	if err != nil {
		return nil, mapErr(err)
	}
	defer rows.Close()
	var out []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, mapErr(err)
		}
		out = append(out, id)
	}
	return out, mapErr(rows.Err())
}

func (s *S3Store) ListAccounts(ctx context.Context) ([]string, error) {
	var out []string
	var token *string
	for {
		res, err := s.client.ListObjectsV2(ctx, &s3.ListObjectsV2Input{
			Bucket: aws.String(s.bucket), Prefix: aws.String(s3KeyPrefix), Delimiter: aws.String("/"), ContinuationToken: token,
		})
		if err != nil {
			return nil, mapS3Err(err)
		}
		for _, p := range res.CommonPrefixes {
			id := strings.Trim(strings.TrimPrefix(aws.ToString(p.Prefix), s3KeyPrefix), "/")
			if ValidateAccountID(id) == nil {
				out = append(out, id)
			}
		}
		if !aws.ToBool(res.IsTruncated) {
			return out, nil
		}
		token = res.NextContinuationToken
	}
}

// CopyReport says what CopyAll did.
type CopyReport struct {
	Copied  int
	Skipped int // already in the destination with the same size and checksum
	Bytes   int64
	Failed  []string
}

// CopyAll streams every object of src into dst (Open -> Put, never buffering a
// whole object). It is how data moves between providers — local to S3 or back.
// Objects already present with identical content are skipped, so it can be
// rerun; nothing is ever deleted from either side, and switching the configured
// driver does not move data by itself.
func CopyAll(ctx context.Context, src, dst Store, bucket string, progress func(ObjectRef)) (CopyReport, error) {
	var rep CopyReport
	lister, ok := src.(AccountLister)
	if !ok {
		return rep, fmt.Errorf("%w: the source provider cannot list its accounts", ErrUnsupported)
	}
	accounts, err := lister.ListAccounts(ctx)
	if err != nil {
		return rep, err
	}
	for _, acct := range accounts {
		cursor := ""
		for {
			page, err := src.List(ctx, ObjectScope{AccountID: acct, Bucket: bucket}, ListOptions{Cursor: cursor})
			if err != nil {
				return rep, err
			}
			for _, o := range page.Objects {
				if err := ctx.Err(); err != nil {
					return rep, err
				}
				if err := copyOne(ctx, src, dst, o.Ref, &rep); err != nil {
					rep.Failed = append(rep.Failed, fmt.Sprintf("%s: %v", o.Ref, err))
					continue
				}
				if progress != nil {
					progress(o.Ref)
				}
			}
			if page.NextCursor == "" {
				break
			}
			cursor = page.NextCursor
		}
	}
	return rep, nil
}

func copyOne(ctx context.Context, src, dst Store, ref ObjectRef, rep *CopyReport) error {
	srcInfo, err := src.Stat(ctx, ref)
	if err != nil {
		return err
	}
	if have, err := dst.Stat(ctx, ref); err == nil && have.Size == srcInfo.Size && have.SHA256 != "" && have.SHA256 == srcInfo.SHA256 {
		rep.Skipped++
		return nil
	}
	rc, _, err := src.Open(ctx, ref, GetOptions{})
	if err != nil {
		return err
	}
	defer rc.Close()
	got, err := dst.Put(ctx, ref, rc, PutOptions{ContentType: srcInfo.ContentType, Metadata: srcInfo.Metadata})
	if err != nil {
		return err
	}
	if got.Size != srcInfo.Size || (srcInfo.SHA256 != "" && got.SHA256 != "" && got.SHA256 != srcInfo.SHA256) {
		_ = dst.Delete(ctx, ref)
		return fmt.Errorf("%w: copy of %s does not match its source", ErrCorruptData, ref)
	}
	rep.Copied++
	rep.Bytes += got.Size
	return nil
}

// ListScopes returns every account + bucket pair the store holds objects in.
func (s *SQLiteStore) ListScopes(ctx context.Context) ([]ObjectScope, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT DISTINCT account_id, bucket FROM objects ORDER BY account_id, bucket`)
	if err != nil {
		return nil, mapErr(err)
	}
	defer rows.Close()
	var out []ObjectScope
	for rows.Next() {
		var sc ObjectScope
		if err := rows.Scan(&sc.AccountID, &sc.Bucket); err != nil {
			return nil, mapErr(err)
		}
		out = append(out, sc)
	}
	return out, mapErr(rows.Err())
}
