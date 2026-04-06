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
    // Helpers
    // ================================================================

    /// @notice Compute nullifier in Solidity (for verification)
    /// @dev nullifier = keccak256(nullifier_note)
    function computeNullifier(bytes32 nullifierNote) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(nullifierNote));
    }
}
