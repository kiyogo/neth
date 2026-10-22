FROM alpine:3.24

ARG TARGETARCH
ARG NETH_TAG=nightly

RUN addgroup -g 2613 neth && \
    adduser -u 2613 -G neth -D -h /neth neth && \
    chown -R neth:neth /neth

WORKDIR /neth
USER neth:neth

ENV RUST_BACKTRACE=1
EXPOSE 25565

ENTRYPOINT [ "neth" ]

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
    CMD nc -z 127.0.0.1 25565 || exit 1