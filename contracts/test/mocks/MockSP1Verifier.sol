// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// @title MockSP1Verifier
/// @notice Mock SP1 Verifier for testing - always passes unless configured to fail
contract MockSP1Verifier is ISP1Verifier {
    bool public shouldPass = true;

    function setShouldPass(bool _shouldPass) external {
        shouldPass = _shouldPass;
    }

    function verifyProof(
        bytes32,
        bytes calldata,
        bytes calldata
    ) external view override {
        require(shouldPass, "Proof verification failed");
    }
}
