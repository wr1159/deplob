// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {TestToken} from "../src/TestToken.sol";

// ============ Production Deploy ============

contract DeployScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address sp1Verifier = vm.envAddress("SP1_VERIFIER_ADDRESS");
        address teeOperator = vm.envAddress("TEE_OPERATOR_ADDRESS");
        bytes32 withdrawVKey = vm.envBytes32("WITHDRAW_VKEY");

        vm.startBroadcast(deployerPrivateKey);

        DePLOB deplob = new DePLOB(sp1Verifier, withdrawVKey, teeOperator);
        console.log("DePLOB deployed at:", address(deplob));

        vm.stopBroadcast();
    }
}

// ============ Local Deploy (Anvil + MockSP1Verifier) ============

/// @notice Deploy everything locally with mock verifier. Mints test tokens to
///         the first two Anvil default accounts for E2E testing.
contract DeployLocalScript is Script {
    function run() external {
        vm.startBroadcast();

        // Deploy mock verifier
        MockSP1Verifier mockVerifier = new MockSP1Verifier();

        // Deploy test tokens
        TestToken tokenA = new TestToken("Token A", "TKA");
        TestToken tokenB = new TestToken("Token B", "TKB");

        // Deploy DePLOB
        bytes32 withdrawVKey = bytes32(uint256(2)); // dummy
        DePLOB deplob = new DePLOB(
            address(mockVerifier),
            withdrawVKey,
            msg.sender // deployer = TEE operator for local testing
        );

        // Whitelist tokens
        deplob.addSupportedToken(address(tokenA));
        deplob.addSupportedToken(address(tokenB));

        // Mint tokens to Anvil default accounts (index 0 and 1)
        // Anvil account 0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        // Anvil account 1: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
        address user1 = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;
        address user2 = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        uint256 mintAmount = 10_000 * 1e18;

        tokenA.mint(user1, mintAmount);
        tokenA.mint(user2, mintAmount);
        tokenB.mint(user1, mintAmount);
        tokenB.mint(user2, mintAmount);

        console.log("=== Deployed (Local) ===");
        console.log("MockSP1Verifier:", address(mockVerifier));
        console.log("Token A (TKA):", address(tokenA));
        console.log("Token B (TKB):", address(tokenB));
        console.log("DePLOB:", address(deplob));
        console.log("TEE Operator:", msg.sender);

        vm.stopBroadcast();
    }
}

// ============ Sepolia Deploy ============

/// @notice Deploy to Sepolia. Uses mock or real SP1 verifier based on env var.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY  — deployer + owner
///   TEE_OPERATOR_ADDRESS  — TEE wallet address
///
/// Optional env vars:
///   SP1_VERIFIER_ADDRESS  — set to use real SP1 verifier (Succinct gateway)
///   WITHDRAW_VKEY         — required when using real verifier
///   USER1_ADDRESS         — wallet to mint test tokens to
///   USER2_ADDRESS         — second wallet to mint test tokens to
contract DeploySepoliaScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address teeOperator = vm.envAddress("TEE_OPERATOR_ADDRESS");

        vm.startBroadcast(deployerPrivateKey);

        // Determine verifier
        address verifierAddr;
        bytes32 withdrawVKey;

        // Try to read real verifier address; if not set, deploy mock
        try vm.envAddress("SP1_VERIFIER_ADDRESS") returns (address addr) {
            verifierAddr = addr;
            withdrawVKey = vm.envBytes32("WITHDRAW_VKEY");
            console.log("Using REAL SP1 verifier:", verifierAddr);
        } catch {
            MockSP1Verifier mockVerifier = new MockSP1Verifier();
            verifierAddr = address(mockVerifier);
            withdrawVKey = bytes32(uint256(2));
            console.log("Using MOCK SP1 verifier:", verifierAddr);
        }

        // Deploy test tokens
        TestToken tokenA = new TestToken("Token A", "TKA");
        TestToken tokenB = new TestToken("Token B", "TKB");

        // Deploy DePLOB
        DePLOB deplob = new DePLOB(verifierAddr, withdrawVKey, teeOperator);

        // Whitelist tokens
        deplob.addSupportedToken(address(tokenA));
        deplob.addSupportedToken(address(tokenB));

        // Mint tokens to users if addresses provided
        uint256 mintAmount = 10_000 * 1e18;
        try vm.envAddress("USER1_ADDRESS") returns (address user1) {
            tokenA.mint(user1, mintAmount);
            tokenB.mint(user1, mintAmount);
            console.log("Minted to user1:", user1);
        } catch {}
        try vm.envAddress("USER2_ADDRESS") returns (address user2) {
            tokenA.mint(user2, mintAmount);
            tokenB.mint(user2, mintAmount);
            console.log("Minted to user2:", user2);
        } catch {}

        console.log("=== Deployed (Sepolia) ===");
        console.log("Verifier:", verifierAddr);
        console.log("Token A (TKA):", address(tokenA));
        console.log("Token B (TKB):", address(tokenB));
        console.log("DePLOB:", address(deplob));
        console.log("TEE Operator:", teeOperator);
        console.log("Withdraw VKey:", vm.toString(withdrawVKey));

        vm.stopBroadcast();
    }
}

// ============ Mock Verifier ============

/// @notice Minimal mock verifier — always passes. FOR TESTING ONLY.
contract MockSP1Verifier {
    function verifyProof(
        bytes32,
        bytes calldata,
        bytes calldata
    ) external pure {
        // Always passes
    }
}
