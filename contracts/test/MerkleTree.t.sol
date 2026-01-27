// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {MerkleTreeWithHistory} from "../src/MerkleTreeWithHistory.sol";

/// @notice Test wrapper to expose internal _insert function
contract MerkleTreeHarness is MerkleTreeWithHistory {
    function insert(bytes32 leaf) external returns (uint256) {
        return _insert(leaf);
    }

    function hashPair(bytes32 left, bytes32 right) external pure returns (bytes32) {
        return _hashPair(left, right);
    }

    function zeros(uint256 level) external pure returns (bytes32) {
        return _zeros(level);
    }
}

contract MerkleTreeTest is Test {
    MerkleTreeHarness public tree;

    function setUp() public {
        tree = new MerkleTreeHarness();
    }

    function test_InitialState() public view {
        assertEq(tree.nextIndex(), 0);
        assertTrue(tree.isKnownRoot(tree.getLastRoot()));
    }

    function test_ZeroHashConsistency() public view {
        // Level 0 should be zero
        assertEq(tree.zeros(0), bytes32(0));

        // Level 1 should be hash(zero || zero)
        bytes32 level1Expected = keccak256(abi.encodePacked(bytes32(0), bytes32(0)));
        assertEq(tree.zeros(1), level1Expected);

        // Level 2 should be hash(level1 || level1)
        bytes32 level2Expected = keccak256(abi.encodePacked(level1Expected, level1Expected));
        assertEq(tree.zeros(2), level2Expected);
    }

    function test_InsertSingleLeaf() public {
        bytes32 leaf = keccak256("test leaf");
        bytes32 rootBefore = tree.getLastRoot();

        uint256 index = tree.insert(leaf);

        assertEq(index, 0);
        assertEq(tree.nextIndex(), 1);

        // Root should change after insert
        bytes32 rootAfter = tree.getLastRoot();
        assertTrue(rootAfter != rootBefore);

        // Both roots should be known
        assertTrue(tree.isKnownRoot(rootBefore));
        assertTrue(tree.isKnownRoot(rootAfter));
    }

    function test_InsertMultipleLeaves() public {
        bytes32 leaf1 = keccak256("leaf 1");
        bytes32 leaf2 = keccak256("leaf 2");
        bytes32 leaf3 = keccak256("leaf 3");

        uint256 index1 = tree.insert(leaf1);
        bytes32 root1 = tree.getLastRoot();

        uint256 index2 = tree.insert(leaf2);
        bytes32 root2 = tree.getLastRoot();

        uint256 index3 = tree.insert(leaf3);
        bytes32 root3 = tree.getLastRoot();

        // Indices should be sequential
        assertEq(index1, 0);
        assertEq(index2, 1);
        assertEq(index3, 2);
        assertEq(tree.nextIndex(), 3);

        // All roots should be different
        assertTrue(root1 != root2);
        assertTrue(root2 != root3);
        assertTrue(root1 != root3);

        // All roots should be known
        assertTrue(tree.isKnownRoot(root1));
        assertTrue(tree.isKnownRoot(root2));
        assertTrue(tree.isKnownRoot(root3));
    }

    function test_RootHistoryOverflow() public {
        // Insert ROOT_HISTORY_SIZE + 5 leaves
        bytes32[] memory allRoots = new bytes32[](35);

        for (uint256 i = 0; i < 35; i++) {
            tree.insert(keccak256(abi.encodePacked("leaf", i)));
            allRoots[i] = tree.getLastRoot();
        }

        // The first 5 roots should no longer be known (overwritten)
        for (uint256 i = 0; i < 5; i++) {
            assertFalse(tree.isKnownRoot(allRoots[i]));
        }

        // The last ROOT_HISTORY_SIZE roots should be known
        for (uint256 i = 5; i < 35; i++) {
            assertTrue(tree.isKnownRoot(allRoots[i]));
        }
    }

    function test_ZeroRootNotKnown() public view {
        assertFalse(tree.isKnownRoot(bytes32(0)));
    }

    function test_DeterministicRoot() public {
        MerkleTreeHarness tree2 = new MerkleTreeHarness();

        bytes32 leaf = keccak256("same leaf");

        tree.insert(leaf);
        tree2.insert(leaf);

        // Same insertions should produce same root
        assertEq(tree.getLastRoot(), tree2.getLastRoot());
    }

    function test_HashPairOrder() public view {
        bytes32 a = keccak256("a");
        bytes32 b = keccak256("b");

        bytes32 hashAB = tree.hashPair(a, b);
        bytes32 hashBA = tree.hashPair(b, a);

        // Order should matter
        assertTrue(hashAB != hashBA);

        // Should match keccak256(abi.encodePacked())
        assertEq(hashAB, keccak256(abi.encodePacked(a, b)));
        assertEq(hashBA, keccak256(abi.encodePacked(b, a)));
    }

    function testFuzz_InsertEmitsEvent(bytes32 leaf) public {
        // Only check indexed params (leaf, leafIndex), skip checking newRoot value
        vm.expectEmit(true, true, false, false);
        emit MerkleTreeWithHistory.LeafInserted(leaf, 0, bytes32(0));
        tree.insert(leaf);
    }
}
