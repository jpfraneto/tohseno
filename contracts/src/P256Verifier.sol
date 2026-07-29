// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

/// @notice Strict application wrapper for the EIP-7951 P256VERIFY precompile.
/// @dev Signatures are exactly 0x01 || x || y || r || s (129 bytes).
library P256Verifier {
    address internal constant PRECOMPILE = address(0x100);

    uint256 internal constant FIELD_MODULUS = 0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff;
    uint256 internal constant CURVE_B = 0x5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b;
    uint256 internal constant CURVE_ORDER = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551;
    uint256 internal constant HALF_CURVE_ORDER = 0x7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8;

    uint256 internal constant SIGNATURE_LENGTH = 129;
    uint8 internal constant SIGNATURE_VERSION = 1;

    struct Components {
        uint256 x;
        uint256 y;
        uint256 r;
        uint256 s;
    }

    function keyId(uint256 x, uint256 y) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(x, y));
    }

    function validPublicKey(uint256 x, uint256 y) internal pure returns (bool) {
        if (x >= FIELD_MODULUS || y >= FIELD_MODULUS || (x == 0 && y == 0)) return false;
        uint256 left = mulmod(y, y, FIELD_MODULUS);
        uint256 xSquared = mulmod(x, x, FIELD_MODULUS);
        uint256 xCubed = mulmod(xSquared, x, FIELD_MODULUS);
        uint256 minusThreeX = FIELD_MODULUS - mulmod(3, x, FIELD_MODULUS);
        uint256 right = addmod(addmod(xCubed, minusThreeX, FIELD_MODULUS), CURVE_B, FIELD_MODULUS);
        return left == right;
    }

    function decode(bytes calldata signature)
        internal
        pure
        returns (bool validEncoding, Components memory components)
    {
        if (signature.length != SIGNATURE_LENGTH || uint8(signature[0]) != SIGNATURE_VERSION) {
            return (false, components);
        }

        uint256 x;
        uint256 y;
        uint256 r;
        uint256 s;
        assembly ("memory-safe") {
            let offset := signature.offset
            x := calldataload(add(offset, 1))
            y := calldataload(add(offset, 33))
            r := calldataload(add(offset, 65))
            s := calldataload(add(offset, 97))
        }
        components = Components({x: x, y: y, r: r, s: s});

        validEncoding = validPublicKey(x, y) && r != 0 && r < CURVE_ORDER && s != 0 && s <= HALF_CURVE_ORDER;
    }

    function verify(bytes32 digest, bytes calldata signature) internal view returns (bool valid, bytes32 signerKeyId) {
        (bool validEncoding, Components memory components) = decode(signature);
        if (!validEncoding) return (false, bytes32(0));

        signerKeyId = keyId(components.x, components.y);
        bytes memory input = abi.encodePacked(digest, components.r, components.s, components.x, components.y);
        (bool success, bytes memory output) = PRECOMPILE.staticcall(input);
        if (!success || output.length != 32) return (false, signerKeyId);

        uint256 result;
        assembly ("memory-safe") {
            result := mload(add(output, 32))
        }
        valid = result == 1;
    }
}
