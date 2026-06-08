FROM alpine AS builder
ARG RUSTIC_VERSION
ARG TARGETARCH
RUN apk add --no-cache ca-certificates && \
    if [ "$TARGETARCH" = "amd64" ]; then \
        ASSET="rustic-${RUSTIC_VERSION}-x86_64-unknown-linux-musl.tar.gz";\
    elif [ "$TARGETARCH" = "arm64" ]; then \
        ASSET="rustic-${RUSTIC_VERSION}-aarch64-unknown-linux-musl.tar.gz"; \
    fi; \
    wget https://github.com/rustic-rs/rustic/releases/download/${RUSTIC_VERSION}/${ASSET} && \
    tar -xzf ${ASSET} && \
    mkdir /etc_files && \
    touch /etc_files/passwd && \
    touch /etc_files/group

FROM scratch
COPY --from=builder /rustic /
COPY --from=builder /etc_files/ /etc/
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
ENTRYPOINT ["/rustic"]
