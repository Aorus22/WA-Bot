# Build Stage
FROM golang:1.23-alpine AS builder

WORKDIR /app

RUN apk add --no-cache gcc musl-dev sqlite-dev

COPY go.mod go.sum ./
RUN go mod download

COPY . .

RUN CGO_ENABLED=1 GOOS=linux GOARCH=amd64 go build -o wa-bot

# Runtime Stage
FROM frolvlad/alpine-glibc:latest

WORKDIR /root/

RUN apk add --no-cache sqlite-libs libwebp-tools

COPY --from=builder /app/wa-bot .
COPY --from=builder /app/ffmpeg /usr/local/bin
COPY --from=builder /app/ffprobe /usr/local/bin
COPY --from=builder /app/yt-dlp /usr/local/bin
COPY --from=builder /app/gallery-dl /usr/local/bin

RUN chmod +x ./wa-bot /usr/local/bin/ffmpeg /usr/local/bin/gallery-dl /usr/local/bin/yt-dlp /usr/local/bin/ffprobe

CMD ["./wa-bot"]
