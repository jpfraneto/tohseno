// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {TestBase} from "./TestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";

/// @dev Runtime installed at 0x100 in tests. Slot 0 is an optional expected digest.
/// Slot 1 selects output: 0=valid, 1=zero word, 2=31 bytes, 3=empty, 4=revert, 5=word two.
contract ConfigurableP256Precompile {
    fallback() external {
        assembly ("memory-safe") {
            switch sload(1)
            case 1 {
                mstore(0, 0)
                return(0, 32)
            }
            case 2 {
                mstore(0, 1)
                return(1, 31)
            }
            case 3 { return(0, 0) }
            case 4 { revert(0, 0) }
            case 5 {
                mstore(0, 2)
                return(0, 32)
            }
            default {
                if iszero(eq(calldatasize(), 160)) { return(0, 0) }
                let expected := sload(0)
                if and(iszero(iszero(expected)), iszero(eq(calldataload(0), expected))) { return(0, 0) }
                mstore(0, 1)
                return(0, 32)
            }
        }
    }
}

abstract contract ProtocolTestBase is TestBase {
    address internal constant P256_PRECOMPILE = address(0x100);
    uint256 internal constant P256_N = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551;
    uint256 internal constant P256_HALF_N = 0x7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8;
    uint256 internal constant P256_P = 0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff;

    uint256 internal constant KEY1_X = 0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296;
    uint256 internal constant KEY1_Y = 0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5;
    uint256 internal constant KEY2_X = KEY1_X;
    uint256 internal constant KEY2_Y = P256_P - KEY1_Y;
    uint256 internal constant KEY3_X = 0x2927b10512bae3eddcfe467828128bad2903269919f7086069c8c4df6c732838;
    uint256 internal constant KEY3_Y = 0xc7787964eaac00e5921fb1498a60f4606766b3d9685001558d1a974e7341513e;

    function installP256Mock() internal {
        ConfigurableP256Precompile implementation = new ConfigurableP256Precompile();
        vm.etch(P256_PRECOMPILE, address(implementation).code);
        setP256Expected(bytes32(0));
        setP256Mode(0);
    }

    function setP256Expected(bytes32 digest) internal {
        vm.store(P256_PRECOMPILE, bytes32(uint256(0)), digest);
    }

    function setP256Mode(uint256 mode) internal {
        vm.store(P256_PRECOMPILE, bytes32(uint256(1)), bytes32(mode));
    }

    function p256Signature(uint256 x, uint256 y) internal pure returns (bytes memory) {
        return abi.encodePacked(bytes1(0x01), x, y, uint256(1), uint256(1));
    }

    function p256SignatureWithS(uint256 x, uint256 y, uint256 s) internal pure returns (bytes memory) {
        return abi.encodePacked(bytes1(0x01), x, y, uint256(1), s);
    }

    function recoverySignature(uint256 privateKey, bytes32 digest) internal returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, digest);
        return abi.encodePacked(r, s, v);
    }

    function newAccount(uint256 x, uint256 y) internal returns (BuilderAccount) {
        return new BuilderAccount(x, y);
    }
}
