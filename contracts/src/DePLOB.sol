// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {MerkleTreeWithHistory} from "./MerkleTreeWithHistory.sol";
import {IDePLOB} from "./interfaces/IDePLOB.sol";
import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @notice Minimal interface for Automata's on-chain DCAP attestation verifier
interface IAutomataDcapVerifier {
    function verifyAndAttestOnChain(bytes calldata rawQuote)
        external
        payable
        returns (bool success, bytes memory output);
}

/// @title DePLOB
/// @notice Decentralized Private Limit Order Book - Shielded Pool Contract
/// @dev Implements privacy-preserving deposits, withdrawals, and order management
contract DePLOB is IDePLOB, MerkleTreeWithHistory, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ============ Constants ============

    /// @notice SP1 program verification key (withdrawal only)
    bytes32 public immutable WITHDRAW_VKEY;

    // ============ State Variables ============

    /// @notice SP1 verifier contract
    ISP1Verifier public immutable verifier;

    /// @notice Spent nullifiers (prevents double-spend)
    mapping(bytes32 => bool) public nullifierHashes;

    /// @notice Known commitments (for validation)
    mapping(bytes32 => bool) public commitments;

    /// @notice Whitelisted tokens
    mapping(address => bool) public supportedTokens;

    /// @notice TEE operator address (can call settleMatch)
    address public teeOperator;

    /// @notice Enclave signing key for attestation verification
    address public enclaveSigningKey;

    /// @notice Whether attestation is required for settlement
    bool public requireAttestation;

    /// @notice Trusted MRENCLAVE value for enclave registration
    bytes32 public trustedMrEnclave;

    /// @notice Automata DCAP attestation verifier contract
    IAutomataDcapVerifier public dcapVerifier;

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
        bytes32 _withdrawVKey,
        address _teeOperator
    ) {
        verifier = ISP1Verifier(_verifier);
        WITHDRAW_VKEY = _withdrawVKey;
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

    /// @notice Set the enclave signing key for attestation verification
    function setEnclaveSigningKey(address _key) external onlyOwner {
        enclaveSigningKey = _key;
    }

    /// @notice Toggle whether attestation is required for settlement
    function setRequireAttestation(bool _required) external onlyOwner {
        requireAttestation = _required;
    }

    /// @notice Set the trusted MRENCLAVE for enclave registration
    function setTrustedMrEnclave(bytes32 _mrenclave) external onlyOwner {
        trustedMrEnclave = _mrenclave;
    }

    /// @notice Set the Automata DCAP verifier address
    function setDcapVerifier(address _dcapVerifier) external onlyOwner {
        dcapVerifier = IAutomataDcapVerifier(_dcapVerifier);
    }

    /// @notice Transfer ownership
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid owner");
        owner = newOwner;
    }

    // ============ Enclave Registration ============

    /// @notice Register an enclave by verifying a DCAP attestation quote on-chain.
    ///         Extracts MRENCLAVE and signing address from the verified quote.
    /// @param dcapQuote The raw DCAP v3 quote bytes from the enclave
    function registerEnclave(bytes calldata dcapQuote) external payable {
        require(address(dcapVerifier) != address(0), "DCAP verifier not set");
        require(trustedMrEnclave != bytes32(0), "Trusted MRENCLAVE not set");

        // Verify the quote via Automata's on-chain verifier (payable — may charge a fee)
        (bool success, ) = dcapVerifier.verifyAndAttestOnChain{value: msg.value}(dcapQuote);
        require(success, "DCAP verification failed");

        // DCAP v3 quote layout:
        //   Bytes 0-47:   Quote Header (48 bytes)
        //   Bytes 48-431: Report Body (384 bytes)
        //     - Report Body offset 64:  MRENCLAVE (32 bytes) → absolute offset 112
        //     - Report Body offset 320: REPORTDATA (64 bytes) → absolute offset 368
        require(dcapQuote.length >= 432, "Quote too short");

        // Extract MRENCLAVE (32 bytes at absolute offset 112)
        bytes32 mrenclave;
        assembly {
            mrenclave := calldataload(add(dcapQuote.offset, 112))
        }
        require(mrenclave == trustedMrEnclave, "MRENCLAVE mismatch");

        // Extract signing address from first 20 bytes of REPORTDATA (absolute offset 368)
        address signingKey;
        assembly {
            signingKey := shr(96, calldataload(add(dcapQuote.offset, 368)))
        }
        require(signingKey != address(0), "Invalid signing key");

        enclaveSigningKey = signingKey;

        emit EnclaveRegistered(signingKey, mrenclave);
    }

    // ============ Deposit ============

    /// @inheritdoc IDePLOB
    function deposit(
        bytes32 commitment,
        address token,
        uint256 amount
    ) external nonReentrant {
        require(supportedTokens[token], "Token not supported");
        require(!commitments[commitment], "Commitment already exists");
        require(amount > 0, "Amount must be positive");

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

        // Verify SP1 proof — public values must exactly match the SP1 program's
        // commit_slice output: nullifier(32) || root(32) || recipient(20) || token(20)
        // || amount(16 bytes little-endian, bincode u128)
        bytes memory publicValues = abi.encodePacked(
            nullifierHash,                      // 32 bytes
            root,                               // 32 bytes
            bytes20(address(recipient)),             // 20 bytes
            bytes20(token),                          // 20 bytes
            _toLittleEndian128(uint128(amount))  // 16 bytes LE
        );
        verifier.verifyProof(WITHDRAW_VKEY, publicValues, proof);

        // Mark nullifier as spent
        nullifierHashes[nullifierHash] = true;

        // Transfer tokens to recipient
        IERC20(token).safeTransfer(recipient, amount);

        emit Withdrawal(recipient, nullifierHash, address(0), 0);
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

        // Verify TEE attestation (ECDSA signature over settlement hash)
        if (attestation.length > 0) {
            bytes32 settlementHash = keccak256(abi.encodePacked(
                buyerOldNullifier,
                sellerOldNullifier,
                buyerNewCommitment,
                sellerNewCommitment
            ));
            bytes32 ethSignedHash = keccak256(abi.encodePacked(
                "\x19Ethereum Signed Message:\n32",
                settlementHash
            ));

            require(attestation.length == 65, "Invalid attestation length");
            bytes32 r;
            bytes32 s;
            uint8 v;
            assembly {
                r := calldataload(attestation.offset)
                s := calldataload(add(attestation.offset, 32))
                v := byte(0, calldataload(add(attestation.offset, 64)))
            }
            address recovered = ecrecover(ethSignedHash, v, r, s);
            require(recovered != address(0) && recovered == enclaveSigningKey, "Invalid attestation");
        } else {
            require(!requireAttestation, "Attestation required");
        }

        // proof parameter reserved for future use (e.g. ZK settlement proof)
        (proof);

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

    // ============ Internal Helpers ============

    /// @notice Convert a uint128 to 16-byte little-endian representation (matches Rust bincode).
    function _toLittleEndian128(uint128 v) internal pure returns (bytes16) {
        // Reverse the 16 bytes
        uint128 result;
        for (uint256 i = 0; i < 16; i++) {
            result = (result << 8) | uint128(uint8(v & 0xff));
            v >>= 8;
        }
        return bytes16(result);
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
