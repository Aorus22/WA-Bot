# Go Builder Stage
FROM golang:1.25-alpine AS go-builder

WORKDIR /app

RUN apk add --no-cache gcc musl-dev sqlite-dev

COPY go.mod go.sum ./
RUN go mod download

COPY . .

RUN --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=1 GOOS=linux GOARCH=amd64 go build -o wa-bot ./cmd/api/main.go

# Frontend Builder Stage
FROM node:22-alpine AS web-builder

WORKDIR /app

COPY web/package*.json ./
RUN npm ci

COPY web/ ./
RUN npm run build

# Runtime Stage
FROM frolvlad/alpine-glibc:latest

WORKDIR /root/

RUN apk add --no-cache sqlite-libs libwebp-tools curl

COPY --from=go-builder /app/wa-bot .
COPY --from=go-builder /app/bin/ffmpeg /usr/local/bin
COPY --from=go-builder /app/bin/ffprobe /usr/local/bin
COPY --from=go-builder /app/bin/yt-dlp /usr/local/bin
COPY --from=go-builder /app/bin/gallery-dl /usr/local/bin
COPY --from=web-builder /app/dist /root/frontend/dist

RUN chmod +x ./wa-bot /usr/local/bin/ffmpeg /usr/local/bin/gallery-dl /usr/local/bin/yt-dlp /usr/local/bin/ffprobe

CMD ["./wa-bot"]
