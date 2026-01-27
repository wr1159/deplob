fn main() {
    // SP1 program should be pre-built using: cargo prove build
    // Run this in the program directory first:
    // cd ../program && cargo prove build
    //
    // Then copy the ELF:
    // mkdir -p ../program/elf
    // cp target/elf-compilation/riscv32im-succinct-zkvm-elf/release/deposit-program ../program/elf/riscv32im-succinct-zkvm-elf

    // Uncomment below to auto-build (may have workspace conflicts):
    // sp1_helper::build_program("../program");

    println!("cargo:rerun-if-changed=../program/elf/");
}
