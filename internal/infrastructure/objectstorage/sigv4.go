package objectstorage

import (
	"context"
	"crypto/subtle"
	"errors"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/aws/aws-sdk-go-v2/aws"
	v4 "github.com/aws/aws-sdk-go-v2/aws/signer/v4"
)

// maxClockSkew is how far a request's timestamp may differ from ours.
const maxClockSkew = 15 * time.Minute

// sigV4Verifier authenticates S3 requests against one access key. It does not
// implement the signature algorithm: it asks the AWS SDK's own signer to sign
// the request the way the client says it did (same headers, region, date) and
// compares the two signatures in constant time. Both the Authorization header
// and presigned query strings are understood.
type sigV4Verifier struct {
	creds  aws.Credentials
	region string
	signer *v4.Signer
	now    func() time.Time
}

func newSigV4Verifier(accessKey, secret, region string) *sigV4Verifier {
	return &sigV4Verifier{
		creds:  aws.Credentials{AccessKeyID: accessKey, SecretAccessKey: secret},
		region: region,
		signer: v4.NewSigner(func(o *v4.SignerOptions) { o.DisableURIPathEscaping = true }), // S3 paths are not double-encoded
		now:    time.Now,
	}
}

var errAccessDenied = errors.New("access denied")

// requireAmzHeadersSigned fails when the request carries an x-amz-* header the
// signature does not cover: otherwise a holder of a signed request could add,
// say, x-amz-copy-source and turn it into a different operation.
func requireAmzHeadersSigned(r *http.Request, signedList string, mustBeSigned ...string) error {
	signed := map[string]bool{}
	for _, n := range strings.Split(signedList, ";") {
		signed[n] = true
	}
	for name := range r.Header {
		lower := strings.ToLower(name)
		if strings.HasPrefix(lower, "x-amz-") && !signed[lower] {
			return errAccessDenied
		}
	}
	for _, n := range mustBeSigned {
		if !signed[n] {
			return errAccessDenied
		}
	}
	return nil
}

// Verify returns nil when r carries a valid signature for the configured key.
func (v *sigV4Verifier) Verify(r *http.Request) error {
	if strings.HasPrefix(r.Header.Get("Authorization"), "AWS4-HMAC-SHA256") {
		return v.verifyHeader(r)
	}
	if r.URL.Query().Get("X-Amz-Algorithm") == "AWS4-HMAC-SHA256" {
		return v.verifyPresigned(r)
	}
	return errAccessDenied
}

type credentialScope struct {
	accessKey, date, region, service string
}

func parseScope(cred string) (credentialScope, bool) {
	p := strings.Split(cred, "/")
	if len(p) != 5 || p[4] != "aws4_request" {
		return credentialScope{}, false
	}
	return credentialScope{accessKey: p[0], date: p[1], region: p[2], service: p[3]}, true
}

func (v *sigV4Verifier) checkScope(sc credentialScope, signTime time.Time) error {
	if subtle.ConstantTimeCompare([]byte(sc.accessKey), []byte(v.creds.AccessKeyID)) != 1 ||
		sc.service != "s3" || sc.region != v.region || sc.date != signTime.UTC().Format("20060102") {
		return errAccessDenied
	}
	return nil
}

func (v *sigV4Verifier) verifyHeader(r *http.Request) error {
	auth := strings.TrimPrefix(r.Header.Get("Authorization"), "AWS4-HMAC-SHA256")
	fields := map[string]string{}
	for _, part := range strings.Split(auth, ",") {
		k, val, ok := strings.Cut(strings.TrimSpace(part), "=")
		if ok {
			fields[k] = val
		}
	}
	sc, ok := parseScope(fields["Credential"])
	signedList, signature := fields["SignedHeaders"], fields["Signature"]
	if !ok || signedList == "" || signature == "" {
		return errAccessDenied
	}
	signTime, err := time.Parse("20060102T150405Z", r.Header.Get("X-Amz-Date"))
	if err != nil {
		return errAccessDenied
	}
	if d := v.now().Sub(signTime); d > maxClockSkew || d < -maxClockSkew {
		return errAccessDenied
	}
	if err := v.checkScope(sc, signTime); err != nil {
		return err
	}
	payloadHash := r.Header.Get("X-Amz-Content-Sha256")
	if payloadHash == "" {
		return errAccessDenied
	}
	if err := requireAmzHeadersSigned(r, signedList, "host", "x-amz-content-sha256", "x-amz-date"); err != nil {
		return err
	}

	// Rebuild the request with exactly the headers the client signed.
	probe := r.Clone(context.Background())
	probe.Header = http.Header{}
	probe.Host = r.Host
	probe.ContentLength = 0
	for _, name := range strings.Split(signedList, ";") {
		switch name {
		case "host":
		case "content-length":
			probe.ContentLength = r.ContentLength
		default:
			vals, present := r.Header[http.CanonicalHeaderKey(name)]
			if !present {
				return errAccessDenied
			}
			probe.Header[http.CanonicalHeaderKey(name)] = vals
		}
	}
	if err := v.signer.SignHTTP(context.Background(), v.creds, probe, payloadHash, "s3", v.region, signTime); err != nil {
		return errAccessDenied
	}
	expected := probe.Header.Get("Authorization")
	_, mine, _ := strings.Cut(expected, "Signature=")
	// The signer must have used the same header list, or the match is meaningless.
	if !strings.Contains(expected, "SignedHeaders="+signedList+",") || subtle.ConstantTimeCompare([]byte(mine), []byte(signature)) != 1 {
		return errAccessDenied
	}
	return nil
}

func (v *sigV4Verifier) verifyPresigned(r *http.Request) error {
	q := r.URL.Query()
	sc, ok := parseScope(q.Get("X-Amz-Credential"))
	signature, signedList := q.Get("X-Amz-Signature"), q.Get("X-Amz-SignedHeaders")
	if !ok || signature == "" || signedList == "" {
		return errAccessDenied
	}
	signTime, err := time.Parse("20060102T150405Z", q.Get("X-Amz-Date"))
	if err != nil {
		return errAccessDenied
	}
	expires, err := strconv.Atoi(q.Get("X-Amz-Expires"))
	if err != nil || expires < 1 || expires > 7*24*3600 {
		return errAccessDenied
	}
	now := v.now()
	if signTime.After(now.Add(maxClockSkew)) || now.After(signTime.Add(time.Duration(expires)*time.Second)) {
		return errAccessDenied
	}
	if err := v.checkScope(sc, signTime); err != nil {
		return err
	}

	if err := requireAmzHeadersSigned(r, signedList, "host"); err != nil {
		return err
	}

	probe := r.Clone(context.Background())
	probe.Header = http.Header{}
	probe.Host = r.Host
	probe.ContentLength = 0
	probe.Body = nil
	for _, name := range strings.Split(signedList, ";") {
		if name == "host" {
			continue
		}
		vals, present := r.Header[http.CanonicalHeaderKey(name)]
		if !present {
			return errAccessDenied
		}
		probe.Header[http.CanonicalHeaderKey(name)] = vals
	}
	clean := url.Values{}
	for k, vals := range q {
		switch k {
		case "X-Amz-Algorithm", "X-Amz-Credential", "X-Amz-Date", "X-Amz-SignedHeaders", "X-Amz-Signature":
		default:
			clean[k] = vals
		}
	}
	probe.URL = &url.URL{Path: r.URL.Path, RawPath: r.URL.RawPath, RawQuery: clean.Encode()}
	signed, _, err := v.signer.PresignHTTP(context.Background(), v.creds, probe, "UNSIGNED-PAYLOAD", "s3", v.region, signTime)
	if err != nil {
		return errAccessDenied
	}
	u, err := url.Parse(signed)
	if err != nil {
		return errAccessDenied
	}
	if u.Query().Get("X-Amz-SignedHeaders") != signedList ||
		subtle.ConstantTimeCompare([]byte(u.Query().Get("X-Amz-Signature")), []byte(signature)) != 1 {
		return errAccessDenied
	}
	return nil
}
