# Step 9: Testing

## Overview

Comprehensive testing strategy for DePLOB:
1. Unit tests for each component
2. Integration tests for workflows
3. End-to-end tests
4. Fuzzing and invariant testing

## 9.1 Testing Structure

```
tests/
├── unit/
│   ├── merkle_tree_test.rs
│   ├── commitment_test.rs
│   ├── orderbook_test.rs
│   └── matching_test.rs
├── integration/
│   ├── deposit_withdraw_test.rs
│   ├── order_lifecycle_test.rs
│   └── matching_settlement_test.rs
├── e2e/
│   └── full_flow_test.rs
└── fuzz/
    └── invariant_test.sol
```

## 9.2 Foundry Unit Tests

### Merkle Tree Tests

`contracts/test/MerkleTree.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {MerkleTreeWithHistory} from "../src/MerkleTreeWithHistory.sol";

contract MerkleTreeWrapper is MerkleTreeWithHistory {
    function insert(bytes32 leaf) external returns (uint256) {
        return _insert(leaf);
    }
}

contract MerkleTreeTest is Test {
    MerkleTreeWrapper public tree;

    function setUp() public {
        tree = new MerkleTreeWrapper();
    }

    function test_InitialRoot() public view {
        bytes32 root = tree.getLastRoot();
        assertTrue(root != bytes32(0), "Initial root should not be zero");
        assertTrue(tree.isKnownRoot(root), "Initial root should be known");
    }

    function test_InsertUpdatesRoot() public {
        bytes32 initialRoot = tree.getLastRoot();

        bytes32 leaf = keccak256("test leaf");
        tree.insert(leaf);

        bytes32 newRoot = tree.getLastRoot();
        assertTrue(newRoot != initialRoot, "Root should change after insert");
        assertTrue(tree.isKnownRoot(newRoot), "New root should be known");
        assertTrue(tree.isKnownRoot(initialRoot), "Old root should still be known");
    }

    function test_InsertMultiple() public {
        bytes32[] memory leaves = new bytes32[](10);
        bytes32[] memory roots = new bytes32[](10);

        for (uint256 i = 0; i < 10; i++) {
            leaves[i] = keccak256(abi.encodePacked("leaf", i));
            tree.insert(leaves[i]);
            roots[i] = tree.getLastRoot();
        }

        // All roots should be known (within history size)
        for (uint256 i = 0; i < 10; i++) {
            assertTrue(tree.isKnownRoot(roots[i]), "Root should be known");
        }
    }

    function test_RootHistorySize() public {
        bytes32 firstRoot = tree.getLastRoot();

        // Insert more than ROOT_HISTORY_SIZE leaves
        for (uint256 i = 0; i < 35; i++) {
            tree.insert(keccak256(abi.encodePacked(i)));
        }

        // First root should no longer be known (history size is 30)
        assertFalse(tree.isKnownRoot(firstRoot), "Old root should be evicted");
    }

    function test_NextIndex() public {
        assertEq(tree.nextIndex(), 0);

        tree.insert(keccak256("leaf1"));
        assertEq(tree.nextIndex(), 1);

        tree.insert(keccak256("leaf2"));
        assertEq(tree.nextIndex(), 2);
    }

    function testFuzz_Insert(bytes32 leaf) public {
        vm.assume(leaf != bytes32(0));

        bytes32 rootBefore = tree.getLastRoot();
        tree.insert(leaf);
        bytes32 rootAfter = tree.getLastRoot();

        assertTrue(rootBefore != rootAfter, "Root should change");
        assertTrue(tree.isKnownRoot(rootAfter), "New root should be valid");
    }
}
```

### DePLOB Core Tests

`contracts/test/DePLOB.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract DePLOBTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public tokenA;
    ERC20Mock public tokenB;

    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
    address public charlie = makeAddr("charlie");
    address public teeOperator = makeAddr("tee");

    bytes32 constant DEPOSIT_VKEY = bytes32(uint256(1));
    bytes32 constant WITHDRAW_VKEY = bytes32(uint256(2));
    bytes32 constant CREATE_ORDER_VKEY = bytes32(uint256(3));
    bytes32 constant CANCEL_ORDER_VKEY = bytes32(uint256(4));

    event Deposit(bytes32 indexed commitment, uint256 indexed leafIndex, uint256 timestamp);
    event Withdrawal(address indexed recipient, bytes32 indexed nullifierHash, address indexed relayer, uint256 fee);
    event OrderCreated(bytes32 indexed orderCommitment, bytes encryptedOrder, uint256 timestamp);
    event OrderCancelled(bytes32 indexed orderNullifier, uint256 timestamp);
    event TradeSettled(bytes32 indexed buyerNewCommitment, bytes32 indexed sellerNewCommitment, uint256 timestamp);

    function setUp() public {
        verifier = new MockSP1Verifier();

        deplob = new DePLOB(
            address(verifier),
            DEPOSIT_VKEY,
            WITHDRAW_VKEY,
            CREATE_ORDER_VKEY,
            CANCEL_ORDER_VKEY,
            teeOperator
        );

        tokenA = new ERC20Mock("Token A", "TKA");
        tokenB = new ERC20Mock("Token B", "TKB");

        deplob.addSupportedToken(address(tokenA));
        deplob.addSupportedToken(address(tokenB));

        // Fund users
        tokenA.mint(alice, 1000 ether);
        tokenA.mint(bob, 1000 ether);
        tokenB.mint(alice, 1000 ether);
        tokenB.mint(bob, 1000 ether);

        // Approve
        vm.prank(alice);
        tokenA.approve(address(deplob), type(uint256).max);
        vm.prank(alice);
        tokenB.approve(address(deplob), type(uint256).max);

        vm.prank(bob);
        tokenA.approve(address(deplob), type(uint256).max);
        vm.prank(bob);
        tokenB.approve(address(deplob), type(uint256).max);
    }

    // ============ Deposit Tests ============

    function test_Deposit_Success() public {
        bytes32 commitment = keccak256("commitment");
        uint256 amount = 100 ether;

        vm.expectEmit(true, true, false, true);
        emit Deposit(commitment, 0, block.timestamp);

        vm.prank(alice);
        deplob.deposit(commitment, address(tokenA), amount, "");

        assertEq(tokenA.balanceOf(address(deplob)), amount);
        assertTrue(deplob.isKnownRoot(deplob.getLastRoot()));
    }

    function test_Deposit_RevertOnDuplicate() public {
        bytes32 commitment = keccak256("commitment");

        vm.prank(alice);
        deplob.deposit(commitment, address(tokenA), 100 ether, "");

        vm.prank(bob);
        vm.expectRevert("Commitment already exists");
        deplob.deposit(commitment, address(tokenA), 100 ether, "");
    }

    function test_Deposit_RevertOnUnsupportedToken() public {
        ERC20Mock unsupported = new ERC20Mock("Unsupported", "UNS");

        vm.expectRevert("Token not supported");
        deplob.deposit(keccak256("c"), address(unsupported), 100 ether, "");
    }

    function test_Deposit_RevertOnZeroAmount() public {
        vm.expectRevert("Amount must be positive");
        deplob.deposit(keccak256("c"), address(tokenA), 0, "");
    }

    // ============ Withdrawal Tests ============

    function test_Withdraw_Success() public {
        // Setup deposit
        bytes32 commitment = keccak256("commitment");
        vm.prank(alice);
        deplob.deposit(commitment, address(tokenA), 100 ether, "");

        bytes32 root = deplob.getLastRoot();
        bytes32 nullifier = keccak256("nullifier");

        uint256 balanceBefore = tokenA.balanceOf(charlie);

        deplob.withdraw(nullifier, payable(charlie), address(tokenA), 100 ether, root, "");

        assertEq(tokenA.balanceOf(charlie), balanceBefore + 100 ether);
        assertTrue(deplob.isSpentNullifier(nullifier));
    }

    function test_Withdraw_RevertOnDoubleSpend() public {
        bytes32 commitment = keccak256("commitment");
        vm.prank(alice);
        deplob.deposit(commitment, address(tokenA), 100 ether, "");

        bytes32 root = deplob.getLastRoot();
        bytes32 nullifier = keccak256("nullifier");

        deplob.withdraw(nullifier, payable(charlie), address(tokenA), 50 ether, root, "");

        vm.expectRevert("Nullifier already spent");
        deplob.withdraw(nullifier, payable(charlie), address(tokenA), 50 ether, root, "");
    }

    function test_Withdraw_RevertOnInvalidRoot() public {
        vm.prank(alice);
        deplob.deposit(keccak256("c"), address(tokenA), 100 ether, "");

        bytes32 fakeRoot = keccak256("fake");

        vm.expectRevert("Unknown root");
        deplob.withdraw(keccak256("n"), payable(charlie), address(tokenA), 100 ether, fakeRoot, "");
    }

    // ============ Order Tests ============

    function test_CreateOrder_Success() public {
        // Setup deposit
        vm.prank(alice);
        deplob.deposit(keccak256("deposit"), address(tokenA), 100 ether, "");

        bytes32 orderCommitment = keccak256("order");
        bytes32 depositNullifier = keccak256("depositNullifier");
        bytes memory encryptedOrder = abi.encode("encrypted");

        vm.expectEmit(true, false, false, true);
        emit OrderCreated(orderCommitment, encryptedOrder, block.timestamp);

        vm.prank(alice);
        deplob.createOrder(orderCommitment, depositNullifier, encryptedOrder, "");

        assertTrue(deplob.orderNullifiers(depositNullifier));
    }

    function test_CancelOrder_Success() public {
        // Setup deposit and order
        vm.prank(alice);
        deplob.deposit(keccak256("deposit"), address(tokenA), 100 ether, "");

        bytes32 orderCommitment = keccak256("order");
        vm.prank(alice);
        deplob.createOrder(orderCommitment, keccak256("dn"), "", "");

        // Cancel
        bytes32 orderNullifier = keccak256("orderNullifier");

        vm.expectEmit(true, false, false, true);
        emit OrderCancelled(orderNullifier, block.timestamp);

        deplob.cancelOrder(orderNullifier, orderCommitment, "");

        assertTrue(deplob.cancelledOrders(orderNullifier));
    }

    // ============ Settlement Tests ============

    function test_SettleMatch_Success() public {
        // Setup deposits
        vm.prank(alice);
        deplob.deposit(keccak256("buyer_deposit"), address(tokenA), 100 ether, "");

        vm.prank(bob);
        deplob.deposit(keccak256("seller_deposit"), address(tokenB), 100 ether, "");

        // Settle as TEE
        bytes32 buyerOldNullifier = keccak256("buyer_null");
        bytes32 sellerOldNullifier = keccak256("seller_null");
        bytes32 buyerNewCommitment = keccak256("buyer_new");
        bytes32 sellerNewCommitment = keccak256("seller_new");

        vm.expectEmit(true, true, false, true);
        emit TradeSettled(buyerNewCommitment, sellerNewCommitment, block.timestamp);

        vm.prank(teeOperator);
        deplob.settleMatch(
            buyerOldNullifier,
            sellerOldNullifier,
            buyerNewCommitment,
            sellerNewCommitment,
            "",
            ""
        );

        assertTrue(deplob.isSpentNullifier(buyerOldNullifier));
        assertTrue(deplob.isSpentNullifier(sellerOldNullifier));
    }

    function test_SettleMatch_RevertOnNonTEE() public {
        vm.prank(alice);
        vm.expectRevert("Not TEE operator");
        deplob.settleMatch(
            keccak256("1"),
            keccak256("2"),
            keccak256("3"),
            keccak256("4"),
            "",
            ""
        );
    }
}
```

## 9.3 SP1 Program Tests

### Rust Unit Tests

`sp1-programs/lib/deplob-core/src/merkle.rs` (add tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_proof_single_leaf() {
        let mut tree = IncrementalMerkleTree::new();

        let leaf = [1u8; 32];
        tree.insert(leaf);

        let proof = tree.proof(0);
        assert!(proof.verify(&leaf, &tree.root));
    }

    #[test]
    fn test_merkle_proof_multiple_leaves() {
        let mut tree = IncrementalMerkleTree::new();

        let leaves: Vec<[u8; 32]> = (0..10)
            .map(|i| {
                let mut leaf = [0u8; 32];
                leaf[0] = i as u8;
                leaf
            })
            .collect();

        for leaf in &leaves {
            tree.insert(*leaf);
        }

        // Verify all proofs
        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.proof(i as u32);
            assert!(proof.verify(leaf, &tree.root), "Proof failed for leaf {}", i);
        }
    }

    #[test]
    fn test_merkle_proof_wrong_leaf_fails() {
        let mut tree = IncrementalMerkleTree::new();

        tree.insert([1u8; 32]);
        tree.insert([2u8; 32]);

        let proof = tree.proof(0);

        // Verify with wrong leaf should fail
        assert!(!proof.verify(&[99u8; 32], &tree.root));
    }

    #[test]
    fn test_merkle_root_changes() {
        let mut tree = IncrementalMerkleTree::new();

        let root1 = tree.root;
        tree.insert([1u8; 32]);
        let root2 = tree.root;
        tree.insert([2u8; 32]);
        let root3 = tree.root;

        assert_ne!(root1, root2);
        assert_ne!(root2, root3);
        assert_ne!(root1, root3);
    }
}
```

### SP1 Execute Tests

`sp1-programs/deposit/script/src/test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sp1_sdk::ProverClient;

    #[test]
    fn test_deposit_program_execution() {
        let client = ProverClient::new();

        let nullifier_note = [1u8; 32];
        let secret = [2u8; 32];
        let token = [0xABu8; 20];
        let amount: u128 = 1_000_000_000_000_000_000;

        let mut stdin = SP1Stdin::new();
        stdin.write(&nullifier_note);
        stdin.write(&secret);
        stdin.write(&token);
        stdin.write(&amount);

        let (output, _) = client.execute(ELF, stdin).run().unwrap();

        let commitment: [u8; 32] = output.read();
        let token_out: [u8; 20] = output.read();
        let amount_out: u128 = output.read();

        // Verify outputs match inputs
        assert_eq!(token_out, token);
        assert_eq!(amount_out, amount);

        // Verify commitment is deterministic
        let mut stdin2 = SP1Stdin::new();
        stdin2.write(&nullifier_note);
        stdin2.write(&secret);
        stdin2.write(&token);
        stdin2.write(&amount);

        let (output2, _) = client.execute(ELF, stdin2).run().unwrap();
        let commitment2: [u8; 32] = output2.read();

        assert_eq!(commitment, commitment2);
    }
}
```

## 9.4 Integration Tests

`contracts/test/Integration.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract IntegrationTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public usdc;
    ERC20Mock public weth;

    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
    address public teeOperator = makeAddr("tee");

    function setUp() public {
        verifier = new MockSP1Verifier();
        deplob = new DePLOB(
            address(verifier),
            bytes32(uint256(1)),
            bytes32(uint256(2)),
            bytes32(uint256(3)),
            bytes32(uint256(4)),
            teeOperator
        );

        usdc = new ERC20Mock("USDC", "USDC");
        weth = new ERC20Mock("WETH", "WETH");

        deplob.addSupportedToken(address(usdc));
        deplob.addSupportedToken(address(weth));

        _fundUsers();
    }

    function _fundUsers() internal {
        usdc.mint(alice, 10000e6);
        usdc.mint(bob, 10000e6);
        weth.mint(alice, 10 ether);
        weth.mint(bob, 10 ether);

        vm.startPrank(alice);
        usdc.approve(address(deplob), type(uint256).max);
        weth.approve(address(deplob), type(uint256).max);
        vm.stopPrank();

        vm.startPrank(bob);
        usdc.approve(address(deplob), type(uint256).max);
        weth.approve(address(deplob), type(uint256).max);
        vm.stopPrank();
    }

    function test_FullTradingFlow() public {
        // 1. Alice deposits WETH
        bytes32 aliceDeposit = keccak256("alice_weth_deposit");
        vm.prank(alice);
        deplob.deposit(aliceDeposit, address(weth), 1 ether, "");

        // 2. Bob deposits USDC
        bytes32 bobDeposit = keccak256("bob_usdc_deposit");
        vm.prank(bob);
        deplob.deposit(bobDeposit, address(usdc), 2000e6, "");

        // 3. Alice creates sell order (selling WETH for USDC)
        bytes32 aliceOrder = keccak256("alice_sell_order");
        bytes32 aliceDepositNullifier = keccak256("alice_deposit_null");
        vm.prank(alice);
        deplob.createOrder(aliceOrder, aliceDepositNullifier, "", "");

        // 4. Bob creates buy order (buying WETH with USDC)
        bytes32 bobOrder = keccak256("bob_buy_order");
        bytes32 bobDepositNullifier = keccak256("bob_deposit_null");
        vm.prank(bob);
        deplob.createOrder(bobOrder, bobDepositNullifier, "", "");

        // 5. TEE matches and settles
        bytes32 aliceNewCommitment = keccak256("alice_receives_usdc");
        bytes32 bobNewCommitment = keccak256("bob_receives_weth");

        vm.prank(teeOperator);
        deplob.settleMatch(
            aliceDepositNullifier,
            bobDepositNullifier,
            bobNewCommitment,    // Bob gets WETH
            aliceNewCommitment,  // Alice gets USDC
            "",
            ""
        );

        // 6. Verify nullifiers spent
        assertTrue(deplob.isSpentNullifier(aliceDepositNullifier));
        assertTrue(deplob.isSpentNullifier(bobDepositNullifier));

        // 7. Both can withdraw to new addresses
        address aliceNewAddr = makeAddr("alice_new");
        address bobNewAddr = makeAddr("bob_new");

        deplob.withdraw(
            keccak256("alice_withdraw_null"),
            payable(aliceNewAddr),
            address(usdc),
            1800e6, // Alice receives USDC
            deplob.getLastRoot(),
            ""
        );

        deplob.withdraw(
            keccak256("bob_withdraw_null"),
            payable(bobNewAddr),
            address(weth),
            1 ether, // Bob receives WETH
            deplob.getLastRoot(),
            ""
        );

        // 8. Verify balances (simplified - actual amounts depend on trade)
        assertGt(usdc.balanceOf(aliceNewAddr), 0);
        assertGt(weth.balanceOf(bobNewAddr), 0);
    }

    function test_OrderCancellationAndReuse() public {
        // Deposit
        bytes32 deposit = keccak256("deposit");
        vm.prank(alice);
        deplob.deposit(deposit, address(weth), 1 ether, "");

        // Create order
        bytes32 order1 = keccak256("order1");
        bytes32 depositNull1 = keccak256("dn1");
        vm.prank(alice);
        deplob.createOrder(order1, depositNull1, "", "");

        // Cancel order
        bytes32 orderNull1 = keccak256("on1");
        deplob.cancelOrder(orderNull1, order1, "");

        // Should be able to create new order
        // (In real implementation, need to handle deposit unlock)
    }
}
```

## 9.5 Fuzzing and Invariant Tests

`contracts/test/Invariant.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract DePLOBHandler is Test {
    DePLOB public deplob;
    ERC20Mock public token;

    uint256 public totalDeposited;
    uint256 public totalWithdrawn;
    bytes32[] public commitments;

    constructor(DePLOB _deplob, ERC20Mock _token) {
        deplob = _deplob;
        token = _token;
    }

    function deposit(uint256 amount, bytes32 commitment) external {
        amount = bound(amount, 1, 1000 ether);

        // Ensure unique commitment
        for (uint i = 0; i < commitments.length; i++) {
            if (commitments[i] == commitment) return;
        }

        token.mint(address(this), amount);
        token.approve(address(deplob), amount);

        deplob.deposit(commitment, address(token), amount, "");

        totalDeposited += amount;
        commitments.push(commitment);
    }

    function withdraw(uint256 index, bytes32 nullifier) external {
        if (commitments.length == 0) return;
        index = bound(index, 0, commitments.length - 1);

        // Skip if nullifier already spent
        if (deplob.isSpentNullifier(nullifier)) return;

        bytes32 root = deplob.getLastRoot();
        uint256 amount = 100 ether; // Simplified

        try deplob.withdraw(nullifier, payable(address(this)), address(token), amount, root, "") {
            totalWithdrawn += amount;
        } catch {}
    }
}

contract InvariantTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public token;
    DePLOBHandler public handler;

    function setUp() public {
        verifier = new MockSP1Verifier();
        deplob = new DePLOB(
            address(verifier),
            bytes32(uint256(1)),
            bytes32(uint256(2)),
            bytes32(uint256(3)),
            bytes32(uint256(4)),
            makeAddr("tee")
        );

        token = new ERC20Mock("Test", "TST");
        deplob.addSupportedToken(address(token));

        handler = new DePLOBHandler(deplob, token);

        targetContract(address(handler));
    }

    /// @notice Invariant: Contract balance >= totalDeposited - totalWithdrawn
    function invariant_SolvencyCheck() public view {
        uint256 contractBalance = token.balanceOf(address(deplob));
        uint256 expectedMinBalance = handler.totalDeposited() - handler.totalWithdrawn();

        assertGe(contractBalance, expectedMinBalance, "Contract is insolvent");
    }

    /// @notice Invariant: Root is always valid after operations
    function invariant_RootAlwaysValid() public view {
        bytes32 root = deplob.getLastRoot();
        assertTrue(deplob.isKnownRoot(root), "Current root should always be known");
    }
}
```

## 9.6 Run Tests

```bash
# Foundry unit tests
cd contracts
forge test -vvv

# Specific test file
forge test --match-path test/DePLOB.t.sol -vvv

# Specific test function
forge test --match-test test_Deposit_Success -vvv

# With gas report
forge test --gas-report

# Coverage
forge coverage

# Invariant tests (run longer)
forge test --match-contract InvariantTest -vvv

# Fuzz tests with more runs
forge test --fuzz-runs 10000

# SP1 Rust tests
cd ../sp1-programs/lib/deplob-core
cargo test

# SP1 program execution tests
cd ../../deposit/script
cargo test
```

## 9.7 Checklist

- [ ] All unit tests pass
- [ ] Integration tests pass
- [ ] Invariant tests pass with 10k+ runs
- [ ] Fuzz tests pass with 10k+ runs
- [ ] SP1 program tests pass
- [ ] Code coverage > 80%
- [ ] Gas costs documented
- [ ] Edge cases covered
