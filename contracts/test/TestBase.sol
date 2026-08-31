// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

interface Vm {
    struct FuzzSelector {
        address addr;
        bytes4[] selectors;
    }

    function addr(uint256 privateKey) external returns (address);
    function assume(bool condition) external;
    function chainId(uint256 newChainId) external;
    function etch(address target, bytes calldata newRuntimeBytecode) external;
    function expectEmit(bool checkTopic1, bool checkTopic2, bool checkTopic3, bool checkData, address emitter)
        external;
    function expectPartialRevert(bytes4 revertData) external;
    function expectRevert(bytes4 revertData) external;
    function expectRevert(bytes calldata revertData) external;
    function prank(address sender) external;
    function sign(uint256 privateKey, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
    function store(address target, bytes32 slot, bytes32 value) external;
    function targetContract(address target) external;
    function warp(uint256 newTimestamp) external;
}

abstract contract TestBase {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    error AssertionFailed();
    error AssertionEqFailed();

    function assertTrue(bool value) internal pure {
        if (!value) revert AssertionFailed();
    }

    function assertFalse(bool value) internal pure {
        if (value) revert AssertionFailed();
    }

    function assertEq(uint256 left, uint256 right) internal pure {
        if (left != right) revert AssertionEqFailed();
    }

    function assertEq(address left, address right) internal pure {
        if (left != right) revert AssertionEqFailed();
    }

    function assertEq(bytes32 left, bytes32 right) internal pure {
        if (left != right) revert AssertionEqFailed();
    }

    function assertEqBytes4(bytes4 left, bytes4 right) internal pure {
        if (left != right) revert AssertionEqFailed();
    }

    function assertEq(string memory left, string memory right) internal pure {
        if (keccak256(bytes(left)) != keccak256(bytes(right))) revert AssertionEqFailed();
    }
}
