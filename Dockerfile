# Build Stage
FROM golang:1.23-alpine AS builder

WORKDIR /app

RUN apk add --no-cache gcc musl-dev sqlite-dev

COPY go.mod go.sum ./
RUN go mod download

COPY . .

RUN CGO_ENABLED=1 GOOS=linux GOARCH=amd64 go build -o wa-bot

# Runtime Stage
FROM alpine:latest

WORKDIR /root/

RUN apk add --no-cache sqlite-libs libwebp-tools

COPY --from=builder /app/wa-bot .
COPY --from=builder /app/ffmpeg .

RUN chmod +x ./wa-bot && chmod +x ./ffmpeg

CMD ["./wa-bot"]
