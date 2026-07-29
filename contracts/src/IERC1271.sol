// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

interface IERC1271 {
    function isValidSignature(bytes32 digest, bytes calldata signature) external view returns (bytes4);
}
