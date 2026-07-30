// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

/// @notice Fail-closed tombstone for the superseded v0.7 broadcast script.
/// @dev The immutable v0.7.1 tag preserves the historical source for audit.
contract DeployCandidate {
    error ContractGenerationRetired();

    function run() external pure {
        revert ContractGenerationRetired();
    }
}
