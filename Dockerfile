FROM alpine as build

ARG TARGETPLATFORM
COPY binaries /binaries
RUN cp /binaries/${TARGETPLATFORM}/berry-pasty /berry-pasty || (ls -lR /binaries && exit 1)

FROM scratch

WORKDIR /app

ADD Rocket.toml /app
COPY --from=build /berry-pasty /app

EXPOSE 8000

ENTRYPOINT ["/app/berry-pasty"]
