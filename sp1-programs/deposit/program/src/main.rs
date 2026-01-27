//! Deposit Program - proves valid commitment creation
#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    // Read inputs (will implement later)
    let value = sp1_zkvm::io::read::<u64>();

    // Simple test: square the value
    let result = value * value;

    // Commit output
    sp1_zkvm::io::commit(&result);
}