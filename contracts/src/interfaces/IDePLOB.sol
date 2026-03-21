// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title IDePLOB
/// @notice Interface for the DePLOB shielded pool
interface IDePLOB {
    // ============ Events ============

    event Deposit(
        bytes32 indexed commitment,
        uint256 indexed leafIndex,
        uint256 timestamp
    );

    event Withdrawal(
        address indexed recipient,
        bytes32 indexed nullifierHash,
        address indexed relayer,
        uint256 fee
    );

    event TradeSettled(
        bytes32 indexed buyerNewCommitment,
        bytes32 indexed sellerNewCommitment,
        uint256 timestamp
    );

    // ============ Deposit ============

    /// @notice Deposit tokens into shielded pool
    /// @param commitment The commitment hash (keccak256 of nullifier_note||secret||token||amount)
    /// @param token The token address
    /// @param amount The amount to deposit
    function deposit(
        bytes32 commitment,
        address token,
        uint256 amount
    ) external;

    // ============ Withdrawal ============

    /// @notice Withdraw tokens from shielded pool
    /// @param nullifierHash The nullifier to prevent double-spend
    /// @param recipient The recipient address
    /// @param token The token address
    /// @param amount The amount to withdraw
    /// @param root The Merkle root to verify against
    /// @param proof The SP1 proof
    function withdraw(
        bytes32 nullifierHash,
        address payable recipient,
        address token,
        uint256 amount,
        bytes32 root,
        bytes calldata proof
    ) external;

    // ============ Settlement (TEE only) ============

    /// @notice Settle a matched trade
    /// @param buyerOldNullifier Buyer's original deposit nullifier
    /// @param sellerOldNullifier Seller's original deposit nullifier
    /// @param buyerNewCommitment New commitment for buyer
    /// @param sellerNewCommitment New commitment for seller
    /// @param attestation TEE attestation
    /// @param proof Settlement proof
    function settleMatch(
        bytes32 buyerOldNullifier,
        bytes32 sellerOldNullifier,
        bytes32 buyerNewCommitment,
        bytes32 sellerNewCommitment,
        bytes calldata attestation,
        bytes calldata proof
    ) external;

    // ============ Admin Functions ============

    /// @notice Set the enclave signing key for attestation verification
    function setEnclaveSigningKey(address _key) external;

    /// @notice Toggle whether attestation is required for settlement
    function setRequireAttestation(bool _required) external;

    // ============ View Functions ============

    function isKnownRoot(bytes32 root) external view returns (bool);
    function isSpentNullifier(bytes32 nullifier) external view returns (bool);
    function getLastRoot() external view returns (bytes32);
}
