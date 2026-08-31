// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {TohsenoClaimsV1} from "../src/TohsenoClaimsV1.sol";

interface VmClaimsDeployment {
    function startBroadcast() external;
    function stopBroadcast() external;
}

/// @notice Narrow owner-attended deployment path for the additive Claims contract.
/// @dev Deployment alone is inactive. Clients require the separate threshold-signed
///      Claims activation and exact runtime/Registry verification before use.
contract DeployClaimsV1 {
    VmClaimsDeployment private constant VM = VmClaimsDeployment(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant ROBINHOOD_CHAIN_ID = 4663;
    address private constant ACTIVE_SHOT_REGISTRY = 0x3FE6508Ba2660Bc575080024F402C192A2E035A0;

    error WrongChain(uint256 observed);
    error RegistryUnavailable();
    error ConstructorBindingFailed();

    function run() external returns (TohsenoClaimsV1 claims) {
        if (block.chainid != ROBINHOOD_CHAIN_ID) revert WrongChain(block.chainid);
        if (ACTIVE_SHOT_REGISTRY.code.length == 0) revert RegistryUnavailable();

        VM.startBroadcast();
        claims = new TohsenoClaimsV1(ACTIVE_SHOT_REGISTRY);
        VM.stopBroadcast();

        if (address(claims.shotRegistry()) != ACTIVE_SHOT_REGISTRY) {
            revert ConstructorBindingFailed();
        }
    }
}
