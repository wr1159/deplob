// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {TestToken} from "../src/TestToken.sol";

/// @notice Redeploy DePLOB on Sepolia reusing previously deployed test tokens.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY  — deployer + owner
///   TEE_OPERATOR_ADDRESS  — TEE wallet address
///   TOKEN_A               — existing Token A address
///   TOKEN_B               — existing Token B address
///
/// Optional env vars:
///   SP1_VERIFIER_ADDRESS  — real SP1 verifier (deploys mock if unset)
///   WITHDRAW_VKEY         — required when using real verifier
///   USER1_ADDRESS         — wallet to mint test tokens to
///   USER2_ADDRESS         — second wallet to mint test tokens to
///   ENCLAVE_SIGNING_KEY   — TEE enclave signing key
contract DeployWithExistingTokensScript is Script {
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

        // Deploy DePLOB
        DePLOB deplob = new DePLOB(verifierAddr, withdrawVKey, teeOperator);

        // Whitelist existing tokens
        deplob.addSupportedToken(tokenA);
        deplob.addSupportedToken(tokenB);

        // Mint tokens to users if addresses provided
        uint256 mintAmount = 10_000 * 1e18;
        try vm.envAddress("USER1_ADDRESS") returns (address user1) {
            TestToken(tokenA).mint(user1, mintAmount);
            TestToken(tokenB).mint(user1, mintAmount);
            console.log("Minted to user1:", user1);
        } catch {}
        try vm.envAddress("USER2_ADDRESS") returns (address user2) {
            TestToken(tokenA).mint(user2, mintAmount);
            TestToken(tokenB).mint(user2, mintAmount);
            console.log("Minted to user2:", user2);
        } catch {}

        // Set enclave signing key if provided
        try vm.envAddress("ENCLAVE_SIGNING_KEY") returns (address enclaveKey) {
            deplob.setEnclaveSigningKey(enclaveKey);
            deplob.setRequireAttestation(true);
            console.log("Enclave signing key:", enclaveKey);
            console.log("Attestation required: true");
        } catch {
            console.log("ENCLAVE_SIGNING_KEY not set - attestation not required");
        }

        console.log("=== Deployed (Sepolia - Existing Tokens) ===");
        console.log("Verifier:", verifierAddr);
        console.log("Token A:", tokenA);
        console.log("Token B:", tokenB);
        console.log("DePLOB:", address(deplob));
        console.log("TEE Operator:", teeOperator);
        console.log("Withdraw VKey:", vm.toString(withdrawVKey));

        vm.stopBroadcast();
    }
}

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
