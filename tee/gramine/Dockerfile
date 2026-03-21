# DePLOB TEE — SGX Enclave Container
#
# Build:   docker build -t deplob-tee-sgx .
# Run:     docker run --device /dev/sgx_enclave --device /dev/sgx_provision \
#            -e ETH_RPC_URL=... -e DEPLOB_ADDRESS=... -e TEE_PRIVATE_KEY=... \
#            -v deplob-sealed:/app/tee/gramine/sealed \
#            -p 3000:3000 deplob-tee-sgx
#
# The sealed volume persists the attestation signing key across restarts.
# On first boot, the enclave generates a new key and logs the address.

FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install Gramine and Intel SGX DCAP dependencies
RUN apt-get update && apt-get install -y \
    curl gnupg2 software-properties-common build-essential pkg-config \
    libssl-dev \
    && curl -fsSLo /usr/share/keyrings/gramine-keyring.gpg \
       https://packages.gramineproject.io/gramine-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/gramine-keyring.gpg] \
       https://packages.gramineproject.io/ jammy main" \
       > /etc/apt/sources.list.d/gramine.list \
    && curl -fsSL https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key \
       | gpg --dearmor -o /usr/share/keyrings/intel-sgx-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/intel-sgx-keyring.gpg] \
       https://download.01.org/intel-sgx/sgx_repo/ubuntu jammy main" \
       > /etc/apt/sources.list.d/intel-sgx.list \
    && apt-get update && apt-get install -y \
       gramine libsgx-dcap-ql libsgx-dcap-default-qpl \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .

# Build binary and sign enclave
RUN cd tee/gramine && make

# Sealed key directory (mount as volume for persistence)
RUN mkdir -p /app/tee/gramine/sealed

EXPOSE 3000

WORKDIR /app/tee/gramine
ENTRYPOINT ["gramine-sgx", "deplob-tee"]
