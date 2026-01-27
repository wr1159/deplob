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

    event OrderCreated(
        bytes32 indexed orderCommitment,
        bytes encryptedOrder,
        uint256 timestamp
    );

    event OrderCancelled(
        bytes32 indexed orderNullifier,
        uint256 timestamp
    );

    event TradeSettled(
        bytes32 indexed buyerNewCommitment,
        bytes32 indexed sellerNewCommitment,
        uint256 timestamp
    );

    // ============ Deposit ============

    /// @notice Deposit tokens into shielded pool
    /// @param commitment The commitment hash
    /// @param token The token address
    /// @param amount The amount to deposit
    /// @param proof The SP1 proof
    function deposit(
        bytes32 commitment,
        address token,
        uint256 amount,
        bytes calldata proof
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

    // ============ Orders ============

    /// @notice Create a new encrypted order
    /// @param orderCommitment The order commitment
    /// @param depositNullifier The nullifier of the deposit backing the order
    /// @param encryptedOrder The encrypted order data for TEE
    /// @param proof The SP1 proof
    function createOrder(
        bytes32 orderCommitment,
        bytes32 depositNullifier,
        bytes calldata encryptedOrder,
        bytes calldata proof
    ) external;

    /// @notice Cancel an existing order
    /// @param orderNullifier The order nullifier
    /// @param orderCommitment The order commitment
    /// @param proof The SP1 proof
    function cancelOrder(
        bytes32 orderNullifier,
        bytes32 orderCommitment,
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

    // ============ View Functions ============

    function isKnownRoot(bytes32 root) external view returns (bool);
    function isSpentNullifier(bytes32 nullifier) external view returns (bool);
    function getLastRoot() external view returns (bytes32);
}
