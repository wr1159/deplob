//! Deposit Program
//!
//! Proves that a commitment is correctly formed from private inputs.
//!
//! Public Inputs:
//!   - commitment: bytes32
//!   - token: address (bytes20)
//!   - amount: uint128
//!
//! Private Inputs:
//!   - nullifier_note: bytes32
//!   - secret: bytes32

#![no_main]
sp1_zkvm::entrypoint!(main);

use deplob_core::CommitmentPreimage;

pub fn main() {
    // ============ Read Private Inputs ============
    let nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let secret: [u8; 32] = sp1_zkvm::io::read();

    // ============ Read Public Inputs ============
    let token: [u8; 20] = sp1_zkvm::io::read();
    let amount: u128 = sp1_zkvm::io::read();

    // ============ Compute Commitment ============
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token, amount);

    let commitment = preimage.commitment();

    // ============ Commit Public Outputs ============
    // These become the public values verified on-chain
    sp1_zkvm::io::commit(&commitment);
    sp1_zkvm::io::commit(&token);
    sp1_zkvm::io::commit(&amount);
}
