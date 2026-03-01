# Build Stage
FROM golang:1.24-alpine AS builder

WORKDIR /app

RUN apk add --no-cache gcc musl-dev sqlite-dev

COPY go.mod go.sum ./
RUN go mod download

COPY . .

RUN --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=1 GOOS=linux GOARCH=amd64 go build -o wa-bot ./cmd/api/main.go

# Runtime Stage
FROM frolvlad/alpine-glibc:latest

WORKDIR /root/

RUN apk add --no-cache sqlite-libs libwebp-tools

COPY --from=builder /app/wa-bot .
COPY --from=builder /app/bin/ffmpeg /usr/local/bin
COPY --from=builder /app/bin/ffprobe /usr/local/bin
COPY --from=builder /app/bin/yt-dlp /usr/local/bin
COPY --from=builder /app/bin/gallery-dl /usr/local/bin

RUN chmod +x ./wa-bot /usr/local/bin/ffmpeg /usr/local/bin/gallery-dl /usr/local/bin/yt-dlp /usr/local/bin/ffprobe

CMD ["./wa-bot"]
