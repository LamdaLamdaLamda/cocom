FROM rust:latest
LABEL maintainer="LamdaLamdaLamda - https://github.com/LamdaLamdaLamda/cocom"

WORKDIR /usr/src/
COPY . .
RUN apt-get -y update && apt-get -y upgrade
RUN apt-get install -y build-essential curl ca-certificates
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin
RUN git clone https://github.com/LamdaLamdaLamda/cocom
WORKDIR /usr/src/cocom
RUN just build && just install
ENTRYPOINT ["cocom"]