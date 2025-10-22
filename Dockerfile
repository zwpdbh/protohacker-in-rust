# Build stage
FROM rust:1.90.0 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Runtime stage
FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/protohacker-in-rust .


EXPOSE 3003
CMD ["./protohacker-in-rust", "speed-daemon", "--port", "3003"]