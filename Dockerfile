# Can be built in the image though it'll be faster and possible to cache deps.

FROM debian:bookworm-slim
COPY target/release/logging_bot /usr/local/bin/logging_bot
# https://pkgs.alpinelinux.org/packages
#RUN apk add --update util-linux
RUN apt-get update && rm -rf /var/lib/apt/lists/*
CMD logging_bot
