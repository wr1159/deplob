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

/// @notice Deploy with mock verifier for testing
contract DeployMockScript is Script {
    function run() external {
        vm.startBroadcast();

        // Deploy mock verifier (for testing only!)
        MockSP1Verifier mockVerifier = new MockSP1Verifier();

        // Placeholder verification keys
        bytes32 depositVKey = bytes32(uint256(1));
        bytes32 withdrawVKey = bytes32(uint256(2));
        bytes32 createOrderVKey = bytes32(uint256(3));
        bytes32 cancelOrderVKey = bytes32(uint256(4));

        DePLOB deplob = new DePLOB(
            address(mockVerifier),
            depositVKey,
            withdrawVKey,
            createOrderVKey,
            cancelOrderVKey,
            msg.sender // TEE operator is deployer for testing
        );

        console.log("Mock SP1 Verifier deployed at:", address(mockVerifier));
        console.log("DePLOB deployed at:", address(deplob));

        vm.stopBroadcast();
    }
}

/// @notice Minimal mock verifier for deployment script
contract MockSP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata) external pure {
        // Always passes - FOR TESTING ONLY
    }
}
