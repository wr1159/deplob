// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title MerkleTreeWithHistory
/// @notice Incremental Merkle tree with root history for async proof verification
/// @dev Based on Tornado Cash implementation, optimized for gas efficiency
contract MerkleTreeWithHistory {
    // ============ Constants ============

    /// @notice Tree depth (supports 2^20 = ~1M leaves)
    uint256 public constant TREE_DEPTH = 20;

    /// @notice Maximum tree capacity
    uint256 public constant MAX_LEAVES = 2 ** TREE_DEPTH;

    /// @notice Number of historical roots to store
    uint256 public constant ROOT_HISTORY_SIZE = 30;

    // ============ State Variables ============

    /// @notice Current index for next leaf insertion
    uint256 public nextIndex;

    /// @notice Cached subtree roots for efficient insertion
    bytes32[TREE_DEPTH] public filledSubtrees;

    /// @notice Circular buffer of historical roots
    bytes32[ROOT_HISTORY_SIZE] public roots;

    /// @notice Current position in root history buffer
    uint256 public currentRootIndex;

    // ============ Events ============

    event LeafInserted(bytes32 indexed leaf, uint256 indexed leafIndex, bytes32 newRoot);

    // ============ Constructor ============

    constructor() {
        // Initialize with zero hashes
        bytes32 currentZero = bytes32(0);
        for (uint256 i = 0; i < TREE_DEPTH; i++) {
            filledSubtrees[i] = currentZero;
            currentZero = _hashPair(currentZero, currentZero);
        }

        // Set initial root
        roots[0] = currentZero;
    }

    // ============ External Functions ============

    /// @notice Check if a root is known (current or historical)
    /// @param root The root to check
    /// @return True if root is known
    function isKnownRoot(bytes32 root) public view virtual returns (bool) {
        if (root == bytes32(0)) return false;

        uint256 index = currentRootIndex;
        for (uint256 i = 0; i < ROOT_HISTORY_SIZE; i++) {
            if (roots[index] == root) return true;
            if (index == 0) {
                index = ROOT_HISTORY_SIZE - 1;
            } else {
                index--;
            }
        }
        return false;
    }

    /// @notice Get the current Merkle root
    /// @return The current root
    function getLastRoot() public view virtual returns (bytes32) {
        return roots[currentRootIndex];
    }

    // ============ Internal Functions ============

    /// @notice Insert a leaf into the tree
    /// @param leaf The leaf to insert
    /// @return index The index where the leaf was inserted
    function _insert(bytes32 leaf) internal returns (uint256 index) {
        require(nextIndex < MAX_LEAVES, "Merkle tree is full");

        index = nextIndex;
        bytes32 currentHash = leaf;
        uint256 currentIndex = index;

        for (uint256 i = 0; i < TREE_DEPTH; i++) {
            if (currentIndex % 2 == 0) {
                // Left child: store current hash, pair with zero
                filledSubtrees[i] = currentHash;
                currentHash = _hashPair(currentHash, _zeros(i));
            } else {
                // Right child: pair with stored subtree
                currentHash = _hashPair(filledSubtrees[i], currentHash);
            }
            currentIndex /= 2;
        }

        // Update root history
        currentRootIndex = (currentRootIndex + 1) % ROOT_HISTORY_SIZE;
        roots[currentRootIndex] = currentHash;

        nextIndex = index + 1;

        emit LeafInserted(leaf, index, currentHash);
    }

    /// @notice Hash two nodes together
    /// @dev Uses keccak256 for EVM efficiency
    function _hashPair(bytes32 left, bytes32 right) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(left, right));
    }

    /// @notice Get zero hash for a given level
    /// @dev Precomputed for gas efficiency
    function _zeros(uint256 level) internal pure returns (bytes32) {
        if (level == 0) return bytes32(0);
        if (level == 1) return 0xad3228b676f7d3cd4284a5443f17f1962b36e491b30a40b2405849e597ba5fb5;
        if (level == 2) return 0xb4c11951957c6f8f642c4af61cd6b24640fec6dc7fc607ee8206a99e92410d30;
        if (level == 3) return 0x21ddb9a356815c3fac1026b6dec5df3124afbadb485c9ba5a3e3398a04b7ba85;
        if (level == 4) return 0xe58769b32a1beaf1ea27375a44095a0d1fb664ce2dd358e7fcbfb78c26a19344;
        if (level == 5) return 0x0eb01ebfc9ed27500cd4dfc979272d1f0913cc9f66540d7e8005811109e1cf2d;
        if (level == 6) return 0x887c22bd8750d34016ac3c66b5ff102dacdd73f6b014e710b51e8022af9a1968;
        if (level == 7) return 0xffd70157e48063fc33c97a050f7f640233bf646cc98d9524c6b92bcf3ab56f83;
        if (level == 8) return 0x9867cc5f7f196b93bae1e27e6320742445d290f2263827498b54fec539f756af;
        if (level == 9) return 0xcefad4e508c098b9a7e1d8feb19955fb02ba9675585078710969d3440f5054e0;
        if (level == 10) return 0xf9dc3e7fe016e050eff260334f18a5d4fe391d82092319f5964f2e2eb7c1c3a5;
        if (level == 11) return 0xf8b13a49e282f609c317a833fb8d976d11517c571d1221a265d25af778ecf892;
        if (level == 12) return 0x3490c6ceeb450aecdc82e28293031d10c7d73bf85e57bf041a97360aa2c5d99c;
        if (level == 13) return 0xc1df82d9c4b87413eae2ef048f94b4d3554cea73d92b0f7af96e0271c691e2bb;
        if (level == 14) return 0x5c67add7c6caf302256adedf7ab114da0acfe870d449a3a489f781d659e8becc;
        if (level == 15) return 0xda7bce9f4e8618b6bd2f4132ce798cdc7a60e7e1460a7299e3c6342a579626d2;
        if (level == 16) return 0x2733e50f526ec2fa19a22b31e8ed50f23cd1fdf94c9154ed3a7609a2f1ff981f;
        if (level == 17) return 0xe1d3b5c807b281e4683cc6d6315cf95b9ade8641defcb32372f1c126e398ef7a;
        if (level == 18) return 0x5a2dce0a8a7f68bb74560f8f71837c2c2ebbcbf7fffb42ae1896f13f7c7479a0;
        if (level == 19) return 0xb46a28b6f55540f89444f63de0378e3d121be09e06cc9ded1c20e65876d36aa0;

        revert("Invalid level");
    }
}
