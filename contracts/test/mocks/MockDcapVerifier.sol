// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Mock DCAP verifier for testing registerEnclave
contract MockDcapVerifier {
    bool public shouldPass;

    constructor() {
        shouldPass = true;
    }

    function setShouldPass(bool _pass) external {
        shouldPass = _pass;
    }

    function verifyAndAttestOnChain(bytes calldata)
        external
        payable
        returns (bool success, bytes memory output)
    {
        return (shouldPass, "");
    }
}
