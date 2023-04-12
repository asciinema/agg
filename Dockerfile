FROM rust:buster as builder

COPY . /usr/src/

WORKDIR /usr/src

RUN ["cargo", "build", "-r"]

FROM rust:buster

LABEL org.opencontainers.image.authors="kayvan.sylvan@gmail.com"

COPY --from=builder /usr/src/target/release/agg /usr/local/bin/

ENTRYPOINT [ "/usr/local/bin/agg" ]

