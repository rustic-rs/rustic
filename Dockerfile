FROM alpine AS builder
ARG RUSTIC_VERSION
RUN wget https://github.com/rustic-rs/rustic/releases/download/${RUSTIC_VERSION}/rustic-${RUSTIC_VERSION}-x86_64-unknown-linux-musl.tar.gz && \
    tar -xzf rustic-${RUSTIC_VERSION}-x86_64-unknown-linux-musl.tar.gz && \
    mkdir /etc_files && \
    touch /etc_files/passwd && \
    touch /etc_files/group

FROM scratch
COPY --from=builder /rustic /
COPY --from=builder /etc_files/ /etc/
ENTRYPOINT ["/rustic"]
