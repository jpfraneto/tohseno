// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {BuilderAccount} from "./BuilderAccount.sol";

/// @notice Permissionless deterministic factory. It never controls created accounts.
contract BuilderAccountFactory {
    event AccountCreated(address indexed account, bytes32 indexed salt, bytes32 indexed initialKeyId);

    function createAccount(bytes32 salt, uint256 initialX, uint256 initialY)
        external
        returns (BuilderAccount account)
    {
        address predicted = predictAccount(salt, initialX, initialY);
        if (predicted.code.length != 0) return BuilderAccount(predicted);

        account = new BuilderAccount{salt: salt}(initialX, initialY);
        emit AccountCreated(address(account), salt, account.deviceKeyId(initialX, initialY));
    }

    function predictAccount(bytes32 salt, uint256 initialX, uint256 initialY) public view returns (address predicted) {
        bytes memory initCode = abi.encodePacked(type(BuilderAccount).creationCode, abi.encode(initialX, initialY));
        bytes32 hash = keccak256(abi.encodePacked(bytes1(0xff), address(this), salt, keccak256(initCode)));
        predicted = address(uint160(uint256(hash)));
    }
}
