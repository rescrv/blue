FROM rust:latest
RUN apt-get update && apt-get upgrade -y && apt-get install -y build-essential cmake libclang-dev
WORKDIR /src
COPY . .
RUN cargo build --all-targets
