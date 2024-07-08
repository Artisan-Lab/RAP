FROM ubuntu:latest
LABEL authors="vaynnecol"

ENV PATH=/root/.cargo/bin:${PATH} \
    HOST_TRIPLE=x86_64-unknown-linux-gnu

RUN apt-get update && \
    apt-get install -y\
    curl \
    build-essential \
    git \
    ninja-build \
    clang \
    python3 \
    z3 \
    make \
    cmake

RUN git --version; \
    ninja --version; \
    clang++ --version; \
    python3 --version; \
    z3 --version; \
    make --version; \
    cmake --version;

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; \
    rustup --version;

WORKDIR /app

RUN git clone https://github.com/Artisan-Lab/RAP.git .

WORKDIR /app/rap

RUN git submodule update --init --recursive; \
    cp ./config.toml ./rust/

WORKDIR /app/rap/rust

RUN ./x.py build --stage 2 compiler/rustc; \
    rustup toolchain link stage2 build/${HOST_TRIPLE}/stage2; \

ENTRYPOINT ["top", "-b"]
