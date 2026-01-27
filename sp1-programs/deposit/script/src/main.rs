use sp1_sdk::{ProverClient, SP1Stdin};

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

fn main() -> anyhow::Result<()> {
    // Initialize prover
    let client = ProverClient::new();

    // Setup inputs
    let mut stdin = SP1Stdin::new();
    stdin.write(&64u64);

    // Execute (without proving, for testing)
    let (mut output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!("Execution report: {:?}", report);

    // Read output
    let result = output.read::<u64>();
    println!("Result: {} (expected: {})", result, 64 * 64);

    // Generate actual proof (uncomment when ready)
    // let (pk, vk) = client.setup(ELF);
    // let proof = client.prove(&pk, stdin).run()?;
    // println!("Proof generated!");

    Ok(())
}
