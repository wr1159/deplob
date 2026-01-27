# Step 3: Smart Contract Foundation

## Overview

Build the foundational smart contracts using Foundry:
1. Incremental Merkle Tree
2. SP1 Verifier integration
3. Base DePLOB contract structure

## 3.1 Project Structure

```
contracts/
├── src/
│   ├── DePLOB.sol              # Main contract
│   ├── MerkleTreeWithHistory.sol
│   ├── interfaces/
│   │   ├── IDePLOB.sol
│   │   └── ISP1Verifier.sol
│   └── libraries/
│       └── PoseidonT3.sol      # Poseidon hash (if needed)
├── test/
│   ├── DePLOB.t.sol
│   └── MerkleTree.t.sol
├── script/
│   └── Deploy.s.sol
└── foundry.toml
```

## 3.2 Incremental Merkle Tree

`contracts/src/MerkleTreeWithHistory.sol`:

```solidity
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
    function isKnownRoot(bytes32 root) public view returns (bool) {
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
    function getLastRoot() public view returns (bytes32) {
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
        // Precomputed zero hashes
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
        if (level == 19) return 0xb46a28b6f55540f89444f63de0378e3d121be09e06cc9ber80ea96e0eb46e3ce5a7f2d6c4bca;

        revert("Invalid level");
    }
}
```

## 3.3 SP1 Verifier Integration

`contracts/src/interfaces/ISP1Verifier.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title ISP1Verifier
/// @notice Interface for SP1 proof verification
interface ISP1Verifier {
    /// @notice Verify an SP1 proof
    /// @param programVKey The verification key for the program
    /// @param publicValues The public values (outputs) from the proof
    /// @param proofBytes The proof bytes
    function verifyProof(
        bytes32 programVKey,
        bytes memory publicValues,
        bytes calldata proofBytes
    ) external view;
}
```

## 3.4 DePLOB Interface

`contracts/src/interfaces/IDePLOB.sol`:

```solidity
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
```

## 3.5 Main DePLOB Contract

`contracts/src/DePLOB.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {MerkleTreeWithHistory} from "./MerkleTreeWithHistory.sol";
import {IDePLOB} from "./interfaces/IDePLOB.sol";
import {ISP1Verifier} from "./interfaces/ISP1Verifier.sol";
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
        require(!orderNullifiers[depositNullifier], "Deposit already used for order");

        // Verify SP1 proof
        bytes memory publicValues = abi.encode(orderCommitment, depositNullifier);
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

        // Spend old nullifiers
        nullifierHashes[buyerOldNullifier] = true;
        nullifierHashes[sellerOldNullifier] = true;

        // Add new commitments
        _insert(buyerNewCommitment);
        _insert(sellerNewCommitment);
        commitments[buyerNewCommitment] = true;
        commitments[sellerNewCommitment] = true;

        emit TradeSettled(buyerNewCommitment, sellerNewCommitment, block.timestamp);
    }

    // ============ View Functions ============

    /// @inheritdoc IDePLOB
    function isSpentNullifier(bytes32 nullifier) external view returns (bool) {
        return nullifierHashes[nullifier];
    }
}
```

## 3.6 Foundry Tests

`contracts/test/MerkleTree.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {MerkleTreeWithHistory} from "../src/MerkleTreeWithHistory.sol";

contract MerkleTreeTest is Test {
    MerkleTreeWithHistory public tree;

    function setUp() public {
        tree = new MerkleTreeWithHistory();
    }

    function test_InitialState() public view {
        assertEq(tree.nextIndex(), 0);
        assertTrue(tree.isKnownRoot(tree.getLastRoot()));
    }

    function test_InsertSingleLeaf() public {
        bytes32 leaf = keccak256("test leaf");

        // Get initial root
        bytes32 rootBefore = tree.getLastRoot();

        // Insert should be done through child contract
        // For testing, we need to create a test wrapper
    }

    function test_RootHistory() public {
        // Test that old roots remain valid
    }
}
```

`contracts/test/DePLOB.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {ISP1Verifier} from "../src/interfaces/ISP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

/// @notice Mock SP1 Verifier for testing
contract MockSP1Verifier is ISP1Verifier {
    bool public shouldPass = true;

    function setShouldPass(bool _shouldPass) external {
        shouldPass = _shouldPass;
    }

    function verifyProof(
        bytes32,
        bytes memory,
        bytes calldata
    ) external view override {
        require(shouldPass, "Proof verification failed");
    }
}

contract DePLOBTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public token;

    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
    address public teeOperator = makeAddr("tee");

    bytes32 constant DEPOSIT_VKEY = bytes32(uint256(1));
    bytes32 constant WITHDRAW_VKEY = bytes32(uint256(2));
    bytes32 constant CREATE_ORDER_VKEY = bytes32(uint256(3));
    bytes32 constant CANCEL_ORDER_VKEY = bytes32(uint256(4));

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

    function test_Deposit() public {
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");

        // Verify state
        assertTrue(deplob.isKnownRoot(deplob.getLastRoot()));
        assertEq(token.balanceOf(address(deplob)), amount);
    }

    function test_DepositRevertsOnDuplicateCommitment() public {
        bytes32 commitment = keccak256("test commitment");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");

        vm.prank(bob);
        vm.expectRevert("Commitment already exists");
        deplob.deposit(commitment, address(token), amount, "");
    }

    function test_DepositRevertsOnUnsupportedToken() public {
        ERC20Mock unsupportedToken = new ERC20Mock("Unsupported", "UNS");
        bytes32 commitment = keccak256("test commitment");

        vm.prank(alice);
        vm.expectRevert("Token not supported");
        deplob.deposit(commitment, address(unsupportedToken), 100 ether, "");
    }

    function test_WithdrawRevertsOnUnknownRoot() public {
        bytes32 nullifier = keccak256("nullifier");
        bytes32 fakeRoot = keccak256("fake root");

        vm.expectRevert("Unknown root");
        deplob.withdraw(nullifier, payable(alice), address(token), 100 ether, fakeRoot, "");
    }

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
}
```

`contracts/test/mocks/ERC20Mock.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract ERC20Mock is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}
```

## 3.7 Deployment Script

`contracts/script/Deploy.s.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {DePLOB} from "../src/DePLOB.sol";

contract DeployScript is Script {
    function run() external {
        // Load environment variables
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address sp1Verifier = vm.envAddress("SP1_VERIFIER_ADDRESS");
        address teeOperator = vm.envAddress("TEE_OPERATOR_ADDRESS");

        // Verification keys (from SP1 setup)
        bytes32 depositVKey = vm.envBytes32("DEPOSIT_VKEY");
        bytes32 withdrawVKey = vm.envBytes32("WITHDRAW_VKEY");
        bytes32 createOrderVKey = vm.envBytes32("CREATE_ORDER_VKEY");
        bytes32 cancelOrderVKey = vm.envBytes32("CANCEL_ORDER_VKEY");

        vm.startBroadcast(deployerPrivateKey);

        DePLOB deplob = new DePLOB(
            sp1Verifier,
            depositVKey,
            withdrawVKey,
            createOrderVKey,
            cancelOrderVKey,
            teeOperator
        );

        console.log("DePLOB deployed at:", address(deplob));

        vm.stopBroadcast();
    }
}
```

## 3.8 Build and Test

```bash
cd contracts

# Build
forge build

# Run tests
forge test -vvv

# Run specific test
forge test --match-test test_Deposit -vvv

# Gas report
forge test --gas-report

# Coverage
forge coverage
```

## 3.9 Checklist

- [ ] MerkleTreeWithHistory compiles
- [ ] DePLOB contract compiles
- [ ] All interfaces defined
- [ ] Mock verifier for testing
- [ ] Basic tests pass
- [ ] Gas costs acceptable
- [ ] Deployment script works
