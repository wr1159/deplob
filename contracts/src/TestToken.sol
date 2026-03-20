// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @title TestToken
/// @notice Simple mintable ERC20 for testing on Anvil/Sepolia.
contract TestToken is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}

    /// @notice Anyone can mint — for testing only.
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}
