FROM rust:1.77 as builder

WORKDIR /app
COPY . .
RUN cargo build --release -p deplob-tee

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/deplob-tee /usr/local/bin/

EXPOSE 3000
ENTRYPOINT ["deplob-tee"]
