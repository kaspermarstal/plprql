FROM postgres:18-bookworm AS builder

RUN apt-get update && apt-get install -y wget
RUN wget -P /tmp https://github.com/kaspermarstal/plprql/releases/download/v1.0.0/plprql-1.0.0-postgresql-18-debian-bookworm-amd64.deb

FROM postgres:18-bookworm

COPY --from=builder /tmp/plprql-1.0.0-postgresql-18-debian-bookworm-amd64.deb /tmp/plprql-1.0.0-postgresql-18-debian-bookworm-amd64.deb
RUN dpkg -i /tmp/plprql-1.0.0-postgresql-18-debian-bookworm-amd64.deb && \
    rm /tmp/plprql-1.0.0-postgresql-18-debian-bookworm-amd64.deb
