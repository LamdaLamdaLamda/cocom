FROM rust:1.48
LABEL maintainer="LamdaLamdaLamda - https://github.com/LamdaLamdaLamda/cocom"

WORKDIR /usr/src/
COPY . .
RUN apt-get -y update && apt-get -y upgrade
RUN apt-get install -y build-essential
RUN git clone https://github.com/LamdaLamdaLamda/cocom
WORKDIR /usr/src/cocom
CMD [ "make", "build"]