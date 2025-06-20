FROM rust:1.87-bullseye as builder
WORKDIR /usr/src/app
COPY . .
WORKDIR /usr/src/app/ghttping
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/ghttping/target/release/ghttping /usr/local/bin/ghttping
ENTRYPOINT ["ghttping"]
