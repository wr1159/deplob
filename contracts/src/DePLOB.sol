// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {MerkleTreeWithHistory} from "./MerkleTreeWithHistory.sol";
import {IDePLOB} from "./interfaces/IDePLOB.sol";
import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title DePLOB
/// @notice Decentralized Private Limit Order Book - Shielded Pool Contract
/// @dev Implements privacy-preserving deposits, withdrawals, and order management
contract DePLOB is IDePLOB, MerkleTreeWithHistory, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ============ Constants ============

    /// @notice SP1 program verification keys
    bytes32 public immutable DEPOSIT_VKEY;
    bytes32 public immutable WITHDRAW_VKEY;
    bytes32 public immutable CREATE_ORDER_VKEY;
    bytes32 public immutable CANCEL_ORDER_VKEY;

    // ============ State Variables ============

    /// @notice SP1 verifier contract
    ISP1Verifier public immutable verifier;

    /// @notice Spent nullifiers (prevents double-spend)
    mapping(bytes32 => bool) public nullifierHashes;

    /// @notice Order nullifiers (tracks used deposits for orders)
    mapping(bytes32 => bool) public orderNullifiers;

    /// @notice Cancelled order nullifiers
    mapping(bytes32 => bool) public cancelledOrders;

    /// @notice Known commitments (for validation)
    mapping(bytes32 => bool) public commitments;

    /// @notice Whitelisted tokens
    mapping(address => bool) public supportedTokens;

    /// @notice TEE operator address (can call settleMatch)
    address public teeOperator;

    /// @notice Contract owner
    address public owner;

    // ============ Modifiers ============

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    modifier onlyTEE() {
        require(msg.sender == teeOperator, "Not TEE operator");
        _;
    }

    // ============ Constructor ============

    constructor(
        address _verifier,
        bytes32 _depositVKey,
        bytes32 _withdrawVKey,
        bytes32 _createOrderVKey,
        bytes32 _cancelOrderVKey,
        address _teeOperator
    ) {
        verifier = ISP1Verifier(_verifier);
        DEPOSIT_VKEY = _depositVKey;
        WITHDRAW_VKEY = _withdrawVKey;
        CREATE_ORDER_VKEY = _createOrderVKey;
        CANCEL_ORDER_VKEY = _cancelOrderVKey;
        teeOperator = _teeOperator;
        owner = msg.sender;
    }

    // ============ Admin Functions ============

    /// @notice Add supported token
    function addSupportedToken(address token) external onlyOwner {
        supportedTokens[token] = true;
    }

    /// @notice Remove supported token
    function removeSupportedToken(address token) external onlyOwner {
        supportedTokens[token] = false;
    }

    /// @notice Update TEE operator
    function setTEEOperator(address _teeOperator) external onlyOwner {
        teeOperator = _teeOperator;
    }

    /// @notice Transfer ownership
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid owner");
        owner = newOwner;
    }

    // ============ Deposit ============

    /// @inheritdoc IDePLOB
    function deposit(
        bytes32 commitment,
        address token,
        uint256 amount,
        bytes calldata proof
    ) external nonReentrant {
        require(supportedTokens[token], "Token not supported");
        require(!commitments[commitment], "Commitment already exists");
        require(amount > 0, "Amount must be positive");

        // Verify SP1 proof
        bytes memory publicValues = abi.encode(commitment, token, amount);
        verifier.verifyProof(DEPOSIT_VKEY, publicValues, proof);

        // Transfer tokens to contract
        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);

        // Insert commitment into Merkle tree
        uint256 leafIndex = _insert(commitment);
        commitments[commitment] = true;

        emit Deposit(commitment, leafIndex, block.timestamp);
    }

    // ============ Withdrawal ============

    /// @inheritdoc IDePLOB
    function withdraw(
        bytes32 nullifierHash,
        address payable recipient,
        address token,
        uint256 amount,
        bytes32 root,
        bytes calldata proof
    ) external nonReentrant {
        require(!nullifierHashes[nullifierHash], "Nullifier already spent");
        require(isKnownRoot(root), "Unknown root");
        require(supportedTokens[token], "Token not supported");

        // Verify SP1 proof
        bytes memory publicValues = abi.encode(
            nullifierHash,
            recipient,
            token,
            amount,
            root
        );
        verifier.verifyProof(WITHDRAW_VKEY, publicValues, proof);

        // Mark nullifier as spent
        nullifierHashes[nullifierHash] = true;

        // Transfer tokens to recipient
        IERC20(token).safeTransfer(recipient, amount);

        emit Withdrawal(recipient, nullifierHash, address(0), 0);
    }

    // ============ Order Creation ============

    /// @inheritdoc IDePLOB
    function createOrder(
        bytes32 orderCommitment,
        bytes32 depositNullifier,
        bytes calldata encryptedOrder,
        bytes calldata proof
    ) external nonReentrant {
        require(
            !orderNullifiers[depositNullifier],
            "Deposit already used for order"
        );

        // Verify SP1 proof
        bytes memory publicValues = abi.encode(
            orderCommitment,
            depositNullifier
        );
        verifier.verifyProof(CREATE_ORDER_VKEY, publicValues, proof);

        // Mark deposit as locked for order
        orderNullifiers[depositNullifier] = true;

        emit OrderCreated(orderCommitment, encryptedOrder, block.timestamp);
    }

    // ============ Order Cancellation ============

    /// @inheritdoc IDePLOB
    function cancelOrder(
        bytes32 orderNullifier,
        bytes32 orderCommitment,
        bytes calldata proof
    ) external nonReentrant {
        require(!cancelledOrders[orderNullifier], "Order already cancelled");

        // Verify SP1 proof
        bytes memory publicValues = abi.encode(orderNullifier, orderCommitment);
        verifier.verifyProof(CANCEL_ORDER_VKEY, publicValues, proof);

        // Mark order as cancelled
        cancelledOrders[orderNullifier] = true;

        emit OrderCancelled(orderNullifier, block.timestamp);
    }

    // ============ Settlement ============

    /// @inheritdoc IDePLOB
    function settleMatch(
        bytes32 buyerOldNullifier,
        bytes32 sellerOldNullifier,
        bytes32 buyerNewCommitment,
        bytes32 sellerNewCommitment,
        bytes calldata attestation,
        bytes calldata proof
    ) external onlyTEE nonReentrant {
        require(!nullifierHashes[buyerOldNullifier], "Buyer nullifier spent");
        require(!nullifierHashes[sellerOldNullifier], "Seller nullifier spent");

        // TODO: Verify TEE attestation
        // TODO: Verify settlement proof
        // For now, we trust the TEE operator
        (attestation, proof); // Silence unused variable warnings

        // Spend old nullifiers
        nullifierHashes[buyerOldNullifier] = true;
        nullifierHashes[sellerOldNullifier] = true;

        // Add new commitments
        _insert(buyerNewCommitment);
        _insert(sellerNewCommitment);
        commitments[buyerNewCommitment] = true;
        commitments[sellerNewCommitment] = true;

        emit TradeSettled(
            buyerNewCommitment,
            sellerNewCommitment,
            block.timestamp
        );
    }

    // ============ View Functions ============

    /// @inheritdoc IDePLOB
    function isSpentNullifier(bytes32 nullifier) external view returns (bool) {
        return nullifierHashes[nullifier];
    }

    /// @inheritdoc IDePLOB
    function isKnownRoot(
        bytes32 root
    ) public view override(IDePLOB, MerkleTreeWithHistory) returns (bool) {
        return MerkleTreeWithHistory.isKnownRoot(root);
    }

    /// @inheritdoc IDePLOB
    function getLastRoot()
        public
        view
        override(IDePLOB, MerkleTreeWithHistory)
        returns (bytes32)
    {
        return MerkleTreeWithHistory.getLastRoot();
    }
}
