// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {IDePLOB} from "../src/interfaces/IDePLOB.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";
import {SP1MockVerifier} from "@sp1-contracts/SP1MockVerifier.sol";

/// @title WithdrawE2ETest
/// @notice End-to-end tests for the withdrawal flow
/// @dev Tests the full deposit -> withdraw cycle using FFI
contract WithdrawE2ETest is Test {
    DePLOB public deplob;
    SP1MockVerifier public verifier;
    ERC20Mock public token;

    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
    address public teeOperator = makeAddr("tee");

    function setUp() public {
        verifier = new SP1MockVerifier();

        deplob = new DePLOB(
            address(verifier),
            bytes32(uint256(2)), // withdraw vkey
            teeOperator
        );

        token = new ERC20Mock("Test Token", "TEST");
        deplob.addSupportedToken(address(token));

        token.mint(alice, 1000 ether);
        vm.prank(alice);
        token.approve(address(deplob), type(uint256).max);
    }

    // ================================================================
    // Approach 1: Mock verifier (fastest, tests contract logic)
    // ================================================================

    /// @notice Test withdrawal with mock verifier
    /// @dev Demonstrates the withdrawal flow with mocked proofs
    function test_WithdrawWithMockVerifier() public {
        // First deposit
        bytes32 commitment = keccak256("test-commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        // Get the root after deposit
        bytes32 root = deplob.getLastRoot();

        // Create withdrawal params
        bytes32 nullifier = keccak256("test-nullifier");

        // Withdraw to bob
        vm.prank(bob);
        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");

        // Check balances
        assertEq(token.balanceOf(address(deplob)), 0);
        assertEq(token.balanceOf(bob), amount);

        // Check nullifier is spent
        assertTrue(deplob.nullifierHashes(nullifier));
    }

    /// @notice Test double-spend prevention
    function test_CannotDoubleSpend() public {
        bytes32 commitment = keccak256("test-commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);
        bytes32 root = deplob.getLastRoot();

        bytes32 nullifier = keccak256("unique-nullifier");

        // First withdrawal succeeds
        vm.prank(bob);
        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");

        // Add more funds to contract for second attempt
        token.mint(address(deplob), amount);

        // Second withdrawal with same nullifier should fail
        vm.prank(bob);
        vm.expectRevert("Nullifier already spent");
        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");
    }

    /// @notice Test withdrawal with invalid root
    function test_WithdrawInvalidRootReverts() public {
        bytes32 commitment = keccak256("test-commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        bytes32 fakeRoot = keccak256("fake-root");
        bytes32 nullifier = keccak256("test-nullifier");

        vm.prank(bob);
        vm.expectRevert("Unknown root");
        deplob.withdraw(nullifier, payable(bob), address(token), amount, fakeRoot, "");
    }

    // ================================================================
    // Approach 2: FFI + Execute-only (validates full circuit)
    // ================================================================

    /// @notice Test full deposit-withdraw cycle with FFI
    /// @dev Generates real commitment and nullifier via SP1 execution
    function test_WithdrawWithFFI() public {
        // Call Rust binary to generate withdrawal test data
        // This creates a deposit, builds a mock Merkle tree, and derives the nullifier
        string[] memory inputs = new string[](7);
        inputs[0] = "cargo";
        inputs[1] = "run";
        inputs[2] = "--release";
        inputs[3] = "--bin";
        inputs[4] = "generate_withdraw_test_data";
        inputs[5] = "--manifest-path";
        inputs[6] = "../sp1-programs/withdraw/script/Cargo.toml";

        console.log("Calling FFI to generate withdrawal test data...");
        bytes memory result = vm.ffi(inputs);

        assertTrue(result.length > 0, "FFI returned empty result");

        // Parse JSON
        string memory jsonStr = string(result);

        // Extract deposit data
        bytes32 commitment = vm.parseJsonBytes32(jsonStr, ".commitment");
        console.log("Commitment:");
        console.logBytes32(commitment);

        // Extract withdrawal public inputs (these come from SP1 execution)
        bytes32 nullifier = vm.parseJsonBytes32(jsonStr, ".nullifier");
        bytes32 root = vm.parseJsonBytes32(jsonStr, ".root");
        address recipient = vm.parseJsonAddress(jsonStr, ".recipient");

        console.log("Nullifier:");
        console.logBytes32(nullifier);
        console.log("Root:");
        console.logBytes32(root);
        console.log("Recipient:", recipient);

        uint256 amount = 1 ether;

        // Step 1: Deposit with the SP1-generated commitment
        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        // The contract's root won't match the mock tree root from Rust,
        // since the contract has its own Merkle tree state.
        // For this test, we use the contract's actual root.
        bytes32 contractRoot = deplob.getLastRoot();
        assertTrue(deplob.isKnownRoot(contractRoot), "Root should be known");

        // Step 2: Withdraw using the SP1-derived nullifier
        // Note: We use contractRoot, not the mock tree root from Rust
        uint256 bobBalanceBefore = token.balanceOf(bob);

        vm.prank(bob);
        deplob.withdraw(nullifier, payable(bob), address(token), amount, contractRoot, "");

        // Verify withdrawal succeeded
        assertEq(token.balanceOf(bob), bobBalanceBefore + amount);
        assertEq(token.balanceOf(address(deplob)), 0);
        assertTrue(deplob.nullifierHashes(nullifier), "Nullifier should be spent");

        console.log("Full deposit-withdraw cycle completed!");
    }

    /// @notice Test that FFI-generated nullifiers work correctly
    function test_FFIGeneratedNullifierPreventsDoubleSpend() public {
        // Generate two sets of test data
        string[] memory inputs = new string[](7);
        inputs[0] = "cargo";
        inputs[1] = "run";
        inputs[2] = "--release";
        inputs[3] = "--bin";
        inputs[4] = "generate_withdraw_test_data";
        inputs[5] = "--manifest-path";
        inputs[6] = "../sp1-programs/withdraw/script/Cargo.toml";

        bytes memory result = vm.ffi(inputs);
        string memory jsonStr = string(result);

        bytes32 commitment = vm.parseJsonBytes32(jsonStr, ".commitment");
        bytes32 nullifier = vm.parseJsonBytes32(jsonStr, ".nullifier");

        uint256 amount = 1 ether;

        // Deposit
        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);
        bytes32 root = deplob.getLastRoot();

        // First withdraw
        vm.prank(bob);
        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");

        // Add more funds
        token.mint(address(deplob), amount);

        // Try to use same nullifier again - should fail
        vm.prank(bob);
        vm.expectRevert("Nullifier already spent");
        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");
    }

    // ================================================================
    // Helpers
    // ================================================================

    /// @notice Compute nullifier in Solidity (for verification)
    /// @dev nullifier = keccak256(nullifier_note)
    function computeNullifier(bytes32 nullifierNote) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(nullifierNote));
    }
}
