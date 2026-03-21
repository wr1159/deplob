# Build and sign the DePLOB TEE enclave with Gramine SGX
#
# Prerequisites:
#   - Intel SGX DCAP driver installed
#   - Gramine (v1.6+) installed
#   - Rust toolchain installed
#
# Usage:
#   make              # Build binary, generate manifest, sign enclave
#   make clean        # Remove build artifacts
#
# After signing, MRENCLAVE is printed. Register the enclave's signing
# address on-chain: cast send $DEPLOB "setEnclaveSigningKey(address)" <addr>

SGX_SIGNER_KEY ?= enclave-key.pem

.PHONY: all clean

all: deplob-tee.manifest.sgx

# Build the TEE binary (release mode)
deplob-tee:
	cargo build --release --manifest-path ../Cargo.toml
	cp ../../target/release/deplob-tee .

# Generate Gramine manifest from template
deplob-tee.manifest: deplob-tee.manifest.template deplob-tee
	gramine-manifest \
		-Dlog_level=error \
		$< $@

# Generate RSA signing key if it doesn't exist
$(SGX_SIGNER_KEY):
	gramine-sgx-gen-private-key $@

# Sign the enclave (produces .manifest.sgx and .sig)
deplob-tee.manifest.sgx deplob-tee.sig: deplob-tee.manifest deplob-tee $(SGX_SIGNER_KEY)
	gramine-sgx-sign \
		--manifest deplob-tee.manifest \
		--key $(SGX_SIGNER_KEY) \
		--output deplob-tee.manifest.sgx
	@echo ""
	@echo "=== Enclave signed successfully ==="
	@echo "MRENCLAVE can be found in deplob-tee.sig"
	@echo ""

# Create sealed storage directory
sealed:
	mkdir -p sealed

clean:
	rm -f deplob-tee deplob-tee.manifest deplob-tee.manifest.sgx deplob-tee.sig
	rm -rf sealed
