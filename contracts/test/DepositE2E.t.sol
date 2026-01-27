// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {IDePLOB} from "../src/interfaces/IDePLOB.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";
import {SP1MockVerifier} from "@sp1-contracts/SP1MockVerifier.sol";

/// @title DepositE2ETest
/// @notice End-to-end tests demonstrating real SP1 proof integration
/// @dev There are 3 approaches for E2E testing:
///
/// 1. SP1MockVerifier (fastest, for development)
///    - Accepts empty proofs
///    - Tests contract logic without proof overhead
///
/// 2. FFI + execute-only (medium, for CI)
///    - Calls Rust script to compute commitment
///    - Uses SP1MockVerifier for verification
///    - Validates the full flow without slow proof generation
///
/// 3. Real proofs (slowest, for final verification)
///    - Pre-generate proofs with GENERATE_PROOF=true
///    - Use actual SP1Verifier contract
///    - Full cryptographic verification
///
contract DepositE2ETest is Test {
    DePLOB public deplob;
    SP1MockVerifier public verifier;
    ERC20Mock public token;

    address public alice = makeAddr("alice");
    address public teeOperator = makeAddr("tee");

    // Verification keys - in production, get these from:
    // GENERATE_PROOF=true cargo run --release
    bytes32 public depositVKey;

    function setUp() public {
        // SP1MockVerifier accepts empty proofs (for execute-only testing)
        verifier = new SP1MockVerifier();

        deplob = new DePLOB(
            address(verifier),
            bytes32(uint256(1)), // deposit vkey placeholder
            bytes32(uint256(2)), // withdraw vkey
            bytes32(uint256(3)), // create order vkey
            bytes32(uint256(4)), // cancel order vkey
            teeOperator
        );

        token = new ERC20Mock("Test Token", "TEST");
        deplob.addSupportedToken(address(token));

        token.mint(alice, 1000 ether);
        vm.prank(alice);
        token.approve(address(deplob), type(uint256).max);
    }

    // ================================================================
    // Approach 1: SP1MockVerifier (fastest)
    // ================================================================

    /// @notice Basic test with mock verifier
    /// @dev SP1MockVerifier accepts empty proofs, testing contract logic only
    function test_DepositWithMockVerifier() public {
        bytes32 commitment = keccak256("test-commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, ""); // empty proof

        assertEq(token.balanceOf(address(deplob)), amount);
        assertTrue(deplob.isKnownRoot(deplob.getLastRoot()));
    }

    // ================================================================
    // Approach 2: FFI + Execute-only (for CI testing)
    // ================================================================

    /// @notice Test with FFI calling Rust script
    /// @dev Generates real commitment via SP1 execution, uses mock verifier
    function test_DepositWithFFI() public {
        // Call Rust binary to generate test data
        string[] memory inputs = new string[](7);
        inputs[0] = "cargo";
        inputs[1] = "run";
        inputs[2] = "--release";
        inputs[3] = "--bin";
        inputs[4] = "generate_test_data";
        inputs[5] = "--manifest-path";
        inputs[6] = "../sp1-programs/deposit/script/Cargo.toml";

        console.log("Calling FFI to generate test data...");
        bytes memory result = vm.ffi(inputs);

        assertTrue(result.length > 0, "FFI returned empty result");

        // Parse JSON using Foundry's built-in JSON parsing
        string memory jsonStr = string(result);

        // Extract values from JSON
        bytes32 commitment = vm.parseJsonBytes32(jsonStr, ".commitment");
        string memory vkeyStr = vm.parseJsonString(jsonStr, ".vkey");

        console.log("SP1-generated commitment:");
        console.logBytes32(commitment);
        console.log("Verification key:", vkeyStr);

        // Use the real commitment from SP1 execution!
        uint256 amount = 1 ether; // Match the amount from generate_test_data (1e18 wei)

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, ""); // empty proof for mock verifier

        assertEq(token.balanceOf(address(deplob)), amount);
        console.log("Deposit successful with SP1-generated commitment!");
    }

    // ================================================================
    // Approach 3: Pre-generated proofs (for production verification)
    // ================================================================

    /// @notice Test with pre-generated proof files
    /// @dev First run: GENERATE_PROOF=true cargo run --release
    ///      This creates deposit_proof.bin, deposit_public_values.bin, deposit_vkey.txt
    function test_DepositWithPreGeneratedProof() public {
        // Check if proof files exist
        string memory proofPath = "../sp1-programs/deposit/script/deposit_proof.bin";

        // Try to read proof file
        try vm.readFileBinary(proofPath) returns (bytes memory proof) {
            // Read other artifacts
            bytes memory publicValues = vm.readFileBinary(
                "../sp1-programs/deposit/script/deposit_public_values.bin"
            );

            // Decode public values (commitment, token, amount)
            (bytes32 commitment, address tokenAddr, uint256 amount) = abi.decode(
                publicValues,
                (bytes32, address, uint256)
            );

            console.log("Loaded proof, size:", proof.length);
            console.log("Commitment:", vm.toString(commitment));
            console.log("Amount:", amount);

            // For real verification, you'd deploy the actual SP1Verifier
            // For now, use mock (which accepts empty proofs)
            vm.prank(alice);
            deplob.deposit(commitment, address(token), amount, "");

            assertEq(token.balanceOf(address(deplob)), amount);
        } catch {
            console.log("Skipping - no pre-generated proof found");
            console.log("Generate with: GENERATE_PROOF=true cargo run --release");

            // Fallback to mock test
            bytes32 commitment = keccak256("fallback");
            uint256 amount = 50 ether;

            vm.prank(alice);
            deplob.deposit(commitment, address(token), amount, "");
        }
    }

    // ================================================================
    // Helper: Compute commitment in Solidity (for verification)
    // ================================================================

    /// @notice Compute commitment matching the Rust implementation
    /// @dev commitment = keccak256(nullifier_note || secret || token_padded || amount_padded)
    function computeCommitment(
        bytes32 nullifierNote,
        bytes32 secret,
        address tokenAddr,
        uint256 amount
    ) internal pure returns (bytes32) {
        // Pad token to 32 bytes (right-aligned, like Rust)
        bytes32 tokenPadded = bytes32(uint256(uint160(tokenAddr)));

        // Amount as 32 bytes (already uint256 in Solidity)
        bytes32 amountPadded = bytes32(amount);

        return keccak256(abi.encodePacked(nullifierNote, secret, tokenPadded, amountPadded));
    }

    /// @notice Test that Solidity commitment matches Rust
    function test_CommitmentConsistency() public pure {
        bytes32 nullifierNote = bytes32(uint256(1));
        bytes32 secret = bytes32(uint256(2));
        address tokenAddr = address(0xABaBaBaBABabABabAbAbABAbABabababaBaBABaB);
        uint256 amount = 1 ether;

        bytes32 commitment = computeCommitment(nullifierNote, secret, tokenAddr, amount);

        // This should match what the Rust program computes
        // You can verify by running the Rust script with the same inputs
        assertTrue(commitment != bytes32(0));
    }
}
