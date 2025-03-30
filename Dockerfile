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
COPY --from=builder /app/ffmpeg .
COPY --from=builder /app/yt-dlp /usr/bin
COPY --from=builder /app/gallery-dl /usr/bin

RUN chmod +x ./wa-bot && chmod +x ./ffmpeg && chmod +x /usr/bin/gallery-dl && chmod +x /usr/bin/yt-dlp

CMD ["./wa-bot"]
