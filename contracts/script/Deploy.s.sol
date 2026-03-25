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

        // Set up DCAP verifier (Automata on Sepolia)
        address dcapVerifier = 0x27188ABA3a26CBb806eF4C67de9b05D7d792EC10;
        deplob.setDcapVerifier(dcapVerifier);
        console.log("DCAP Verifier:", dcapVerifier);

        // Set trusted MRENCLAVE if provided
        try vm.envBytes32("TRUSTED_MRENCLAVE") returns (bytes32 mrenclave) {
            deplob.setTrustedMrEnclave(mrenclave);
            console.log("Trusted MRENCLAVE:", vm.toString(mrenclave));
        } catch {
            console.log(
                "TRUSTED_MRENCLAVE not set - registerEnclave won't work until set"
            );
        }

        // Set enclave signing key if provided (manual fallback for attestation verification)
        try vm.envAddress("ENCLAVE_SIGNING_KEY") returns (address enclaveKey) {
            deplob.setEnclaveSigningKey(enclaveKey);
            deplob.setRequireAttestation(true);
            console.log("Enclave signing key:", enclaveKey);
            console.log("Attestation required: true");
        } catch {
            console.log(
                "ENCLAVE_SIGNING_KEY not set - use registerEnclave() with DCAP quote"
            );
        }

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

// ============ Sepolia Upgrade (reuse existing tokens) ============

/// @notice Deploy only a new DePLOB contract on Sepolia, reusing existing tokens.
///         Use this when upgrading the contract without redeploying tokens.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY  — deployer + owner
///   TEE_OPERATOR_ADDRESS  — TEE wallet address
///   TOKEN_A               — existing Token A address
///   TOKEN_B               — existing Token B address
///
/// Optional env vars:
///   SP1_VERIFIER_ADDRESS  — real SP1 verifier (deploys mock if not set)
///   WITHDRAW_VKEY         — required when using real verifier
///   TRUSTED_MRENCLAVE     — set to enable registerEnclave()
///   ENCLAVE_SIGNING_KEY   — manual fallback (skips DCAP)
contract UpgradeSepoliaScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address teeOperator = vm.envAddress("TEE_OPERATOR_ADDRESS");
        address tokenA = vm.envAddress("TOKEN_A");
        address tokenB = vm.envAddress("TOKEN_B");

        vm.startBroadcast(deployerPrivateKey);

        // Determine verifier
        address verifierAddr;
        bytes32 withdrawVKey;

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

        // Deploy new DePLOB
        DePLOB deplob = new DePLOB(verifierAddr, withdrawVKey, teeOperator);

        // Whitelist existing tokens
        deplob.addSupportedToken(tokenA);
        deplob.addSupportedToken(tokenB);

        // Set up DCAP verifier (Automata on Sepolia)
        address dcapVerifier = 0x27188ABA3a26CBb806eF4C67de9b05D7d792EC10;
        deplob.setDcapVerifier(dcapVerifier);
        console.log("DCAP Verifier:", dcapVerifier);

        // Set trusted MRENCLAVE if provided
        try vm.envBytes32("TRUSTED_MRENCLAVE") returns (bytes32 mrenclave) {
            deplob.setTrustedMrEnclave(mrenclave);
            console.log("Trusted MRENCLAVE:", vm.toString(mrenclave));
        } catch {
            console.log(
                "TRUSTED_MRENCLAVE not set - registerEnclave won't work until set"
            );
        }

        // Set enclave signing key if provided (manual fallback)
        try vm.envAddress("ENCLAVE_SIGNING_KEY") returns (address enclaveKey) {
            deplob.setEnclaveSigningKey(enclaveKey);
            deplob.setRequireAttestation(true);
            console.log("Enclave signing key:", enclaveKey);
            console.log("Attestation required: true");
        } catch {
            console.log(
                "ENCLAVE_SIGNING_KEY not set - use registerEnclave() with DCAP quote"
            );
        }

        console.log("=== Deployed (Sepolia Upgrade) ===");
        console.log("Verifier:", verifierAddr);
        console.log("Token A:", tokenA);
        console.log("Token B:", tokenB);
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
