fn main() {
    // SP1 program should be pre-built using: cargo prove build
    // Run this in the program directory first:
    // cd ../program && cargo prove build
    //
    // Then the ELF will be at:
    // target/elf-compilation/riscv32im-succinct-zkvm-elf/release/withdraw-program
    //
    // Copy it to the expected location:
    // mkdir -p ../program/elf
    // cp target/elf-compilation/riscv32im-succinct-zkvm-elf/release/withdraw-program ../program/elf/riscv32im-succinct-zkvm-elf

    println!("cargo:rerun-if-changed=../program/elf/");
}
