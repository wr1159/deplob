// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {IDePLOB} from "../src/interfaces/IDePLOB.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";

contract DePLOBTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public token;

    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
    address public teeOperator = makeAddr("tee");
    address public deployer;

    bytes32 constant WITHDRAW_VKEY = bytes32(uint256(2));

    function setUp() public {
        deployer = address(this);

        verifier = new MockSP1Verifier();
        deplob = new DePLOB(address(verifier), WITHDRAW_VKEY, teeOperator);

        token = new ERC20Mock("Test Token", "TEST");

        // Setup
        deplob.addSupportedToken(address(token));
        token.mint(alice, 1000 ether);
        token.mint(bob, 1000 ether);

        vm.prank(alice);
        token.approve(address(deplob), type(uint256).max);

        vm.prank(bob);
        token.approve(address(deplob), type(uint256).max);
    }

    // ============ Deposit Tests ============

    function test_Deposit() public {
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        // Verify state
        assertTrue(deplob.isKnownRoot(deplob.getLastRoot()));
        assertEq(token.balanceOf(address(deplob)), amount);
        assertEq(token.balanceOf(alice), 900 ether);
    }

    function test_DepositEmitsEvent() public {
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.expectEmit(true, true, false, true);
        emit IDePLOB.Deposit(commitment, 0, block.timestamp);

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);
    }

    function test_DepositRevertsOnDuplicateCommitment() public {
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        vm.prank(bob);
        vm.expectRevert("Commitment already exists");
        deplob.deposit(commitment, address(token), amount);
    }

    function test_DepositRevertsOnUnsupportedToken() public {
        ERC20Mock unsupportedToken = new ERC20Mock("Unsupported", "UNS");
        bytes32 commitment = keccak256("test commitment");

        vm.prank(alice);
        vm.expectRevert("Token not supported");
        deplob.deposit(commitment, address(unsupportedToken), 100 ether);
    }

    function test_DepositRevertsOnZeroAmount() public {
        bytes32 commitment = keccak256("test commitment");

        vm.prank(alice);
        vm.expectRevert("Amount must be positive");
        deplob.deposit(commitment, address(token), 0);
    }

    // ============ Withdrawal Tests ============

    function test_Withdraw() public {
        // First deposit
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        // Then withdraw
        bytes32 nullifier = keccak256("nullifier");
        bytes32 root = deplob.getLastRoot();

        deplob.withdraw(
            nullifier,
            payable(bob),
            address(token),
            amount,
            root,
            ""
        );

        // Verify state
        assertTrue(deplob.isSpentNullifier(nullifier));
        assertEq(token.balanceOf(bob), 1100 ether);
        assertEq(token.balanceOf(address(deplob)), 0);
    }

    function test_WithdrawRevertsOnDoubleSpend() public {
        // First deposit
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount);

        // First withdraw succeeds
        bytes32 nullifier = keccak256("nullifier");
        bytes32 root = deplob.getLastRoot();

        deplob.withdraw(
            nullifier,
            payable(bob),
            address(token),
            amount,
            root,
            ""
        );

        // Deposit more
        token.mint(alice, 100 ether);
        bytes32 commitment2 = keccak256("test commitment 2");
        vm.prank(alice);
        deplob.deposit(commitment2, address(token), amount);

        // Second withdraw with same nullifier fails
        bytes32 newRoot = deplob.getLastRoot();
        vm.expectRevert("Nullifier already spent");
        deplob.withdraw(
            nullifier,
            payable(bob),
            address(token),
            amount,
            newRoot,
            ""
        );
    }

    function test_WithdrawRevertsOnUnknownRoot() public {
        bytes32 nullifier = keccak256("nullifier");
        bytes32 fakeRoot = keccak256("fake root");

        vm.expectRevert("Unknown root");
        deplob.withdraw(
            nullifier,
            payable(alice),
            address(token),
            100 ether,
            fakeRoot,
            ""
        );
    }

    // ============ Settlement Tests ============

    function test_SettleMatchOnlyTEE() public {
        vm.prank(alice);
        vm.expectRevert("Not TEE operator");
        deplob.settleMatch(
            bytes32(0),
            bytes32(0),
            bytes32(uint256(1)),
            bytes32(uint256(2)),
            "",
            ""
        );
    }

    function test_SettleMatch() public {
        bytes32 buyerNullifier = keccak256("buyer nullifier");
        bytes32 sellerNullifier = keccak256("seller nullifier");
        bytes32 buyerNewCommitment = keccak256("buyer new commitment");
        bytes32 sellerNewCommitment = keccak256("seller new commitment");

        vm.prank(teeOperator);
        vm.expectEmit(true, true, false, true);
        emit IDePLOB.TradeSettled(
            buyerNewCommitment,
            sellerNewCommitment,
            block.timestamp
        );

        deplob.settleMatch(
            buyerNullifier,
            sellerNullifier,
            buyerNewCommitment,
            sellerNewCommitment,
            "",
            ""
        );

        // Verify nullifiers are spent
        assertTrue(deplob.isSpentNullifier(buyerNullifier));
        assertTrue(deplob.isSpentNullifier(sellerNullifier));
    }

    function test_SettleMatchRevertsOnSpentNullifier() public {
        bytes32 buyerNullifier = keccak256("buyer nullifier");
        bytes32 sellerNullifier = keccak256("seller nullifier");

        // First settlement
        vm.prank(teeOperator);
        deplob.settleMatch(
            buyerNullifier,
            sellerNullifier,
            keccak256("commitment 1"),
            keccak256("commitment 2"),
            "",
            ""
        );

        // Second settlement with same buyer nullifier fails
        vm.prank(teeOperator);
        vm.expectRevert("Buyer nullifier spent");
        deplob.settleMatch(
            buyerNullifier,
            keccak256("different seller"),
            keccak256("commitment 3"),
            keccak256("commitment 4"),
            "",
            ""
        );
    }

    // ============ Attestation Tests ============

    // Private key for vm.sign — corresponds to a known address
    uint256 constant ATTESTATION_PK = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80;

    function _attestationSigner() internal pure returns (address) {
        return vm.addr(ATTESTATION_PK);
    }

    function _signSettlement(
        bytes32 buyerNullifier,
        bytes32 sellerNullifier,
        bytes32 buyerCommitment,
        bytes32 sellerCommitment
    ) internal pure returns (bytes memory) {
        bytes32 settlementHash = keccak256(abi.encodePacked(
            buyerNullifier, sellerNullifier, buyerCommitment, sellerCommitment
        ));
        bytes32 ethSignedHash = keccak256(abi.encodePacked(
            "\x19Ethereum Signed Message:\n32", settlementHash
        ));
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(ATTESTATION_PK, ethSignedHash);
        return abi.encodePacked(r, s, v);
    }

    function test_SettleMatchWithValidAttestation() public {
        bytes32 buyerNullifier = keccak256("attest buyer");
        bytes32 sellerNullifier = keccak256("attest seller");
        bytes32 buyerCommitment = keccak256("attest buyer commit");
        bytes32 sellerCommitment = keccak256("attest seller commit");

        // Set enclave signing key
        deplob.setEnclaveSigningKey(_attestationSigner());

        bytes memory attestation = _signSettlement(
            buyerNullifier, sellerNullifier, buyerCommitment, sellerCommitment
        );

        vm.prank(teeOperator);
        deplob.settleMatch(
            buyerNullifier,
            sellerNullifier,
            buyerCommitment,
            sellerCommitment,
            attestation,
            ""
        );

        assertTrue(deplob.isSpentNullifier(buyerNullifier));
        assertTrue(deplob.isSpentNullifier(sellerNullifier));
    }

    function test_SettleMatchWithInvalidAttestation() public {
        bytes32 buyerNullifier = keccak256("bad attest buyer");
        bytes32 sellerNullifier = keccak256("bad attest seller");
        bytes32 buyerCommitment = keccak256("bad attest buyer commit");
        bytes32 sellerCommitment = keccak256("bad attest seller commit");

        // Set enclave signing key to a DIFFERENT address
        deplob.setEnclaveSigningKey(makeAddr("wrong signer"));

        // Sign with ATTESTATION_PK (won't match enclaveSigningKey)
        bytes memory attestation = _signSettlement(
            buyerNullifier, sellerNullifier, buyerCommitment, sellerCommitment
        );

        vm.prank(teeOperator);
        vm.expectRevert("Invalid attestation");
        deplob.settleMatch(
            buyerNullifier,
            sellerNullifier,
            buyerCommitment,
            sellerCommitment,
            attestation,
            ""
        );
    }

    function test_SettleMatchNoAttestationAllowed() public {
        // requireAttestation defaults to false, so empty attestation should pass
        bytes32 buyerNullifier = keccak256("no attest buyer");
        bytes32 sellerNullifier = keccak256("no attest seller");

        vm.prank(teeOperator);
        deplob.settleMatch(
            buyerNullifier,
            sellerNullifier,
            keccak256("commit 1"),
            keccak256("commit 2"),
            "",
            ""
        );

        assertTrue(deplob.isSpentNullifier(buyerNullifier));
    }

    function test_SettleMatchNoAttestationRequired() public {
        // Enable requireAttestation, then try without attestation
        deplob.setRequireAttestation(true);

        vm.prank(teeOperator);
        vm.expectRevert("Attestation required");
        deplob.settleMatch(
            keccak256("req buyer"),
            keccak256("req seller"),
            keccak256("req commit 1"),
            keccak256("req commit 2"),
            "",
            ""
        );
    }

    function test_SetEnclaveSigningKeyOnlyOwner() public {
        vm.prank(alice);
        vm.expectRevert("Not owner");
        deplob.setEnclaveSigningKey(alice);

        // Owner can set
        deplob.setEnclaveSigningKey(alice);
        assertEq(deplob.enclaveSigningKey(), alice);
    }

    function test_SetRequireAttestationOnlyOwner() public {
        vm.prank(alice);
        vm.expectRevert("Not owner");
        deplob.setRequireAttestation(true);

        // Owner can set
        deplob.setRequireAttestation(true);
        assertTrue(deplob.requireAttestation());
    }

    // ============ Admin Tests ============

    function test_OnlyOwnerCanAddToken() public {
        ERC20Mock newToken = new ERC20Mock("New", "NEW");

        vm.prank(alice);
        vm.expectRevert("Not owner");
        deplob.addSupportedToken(address(newToken));

        // Owner can add
        deplob.addSupportedToken(address(newToken));
        assertTrue(deplob.supportedTokens(address(newToken)));
    }

    function test_OnlyOwnerCanRemoveToken() public {
        vm.prank(alice);
        vm.expectRevert("Not owner");
        deplob.removeSupportedToken(address(token));

        // Owner can remove
        deplob.removeSupportedToken(address(token));
        assertFalse(deplob.supportedTokens(address(token)));
    }

    function test_OnlyOwnerCanSetTEEOperator() public {
        address newTEE = makeAddr("new tee");

        vm.prank(alice);
        vm.expectRevert("Not owner");
        deplob.setTEEOperator(newTEE);

        // Owner can set
        deplob.setTEEOperator(newTEE);
        assertEq(deplob.teeOperator(), newTEE);
    }

    function test_OnlyOwnerCanTransferOwnership() public {
        vm.prank(alice);
        vm.expectRevert("Not owner");
        deplob.transferOwnership(alice);

        // Owner can transfer
        deplob.transferOwnership(alice);
        assertEq(deplob.owner(), alice);
    }

    function test_TransferOwnershipRevertsOnZeroAddress() public {
        vm.expectRevert("Invalid owner");
        deplob.transferOwnership(address(0));
    }

    // ============ View Function Tests ============

    function test_IsKnownRoot() public {
        bytes32 initialRoot = deplob.getLastRoot();
        assertTrue(deplob.isKnownRoot(initialRoot));

        // Insert a commitment
        bytes32 commitment = keccak256("commitment");
        vm.prank(alice);
        deplob.deposit(commitment, address(token), 100 ether);

        bytes32 newRoot = deplob.getLastRoot();
        assertTrue(deplob.isKnownRoot(newRoot));
        assertTrue(deplob.isKnownRoot(initialRoot)); // Old root still known
    }

    function test_IsSpentNullifier() public view {
        bytes32 nullifier = keccak256("random nullifier");
        assertFalse(deplob.isSpentNullifier(nullifier));
    }
}
