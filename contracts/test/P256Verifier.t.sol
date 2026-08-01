// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {P256Verifier} from "../src/P256Verifier.sol";

contract P256VerifierHarness {
    function validPublicKey(uint256 x, uint256 y) external pure returns (bool) {
        return P256Verifier.validPublicKey(x, y);
    }

    function verify(bytes32 digest, bytes calldata signature) external view returns (bool, bytes32) {
        return P256Verifier.verify(digest, signature);
    }
}

contract P256VerifierTest is ProtocolTestBase {
    BuilderAccount private account;
    P256VerifierHarness private harness;

    function setUp() public {
        installP256Mock();
        account = newAccount(KEY1_X, KEY1_Y);
        harness = new P256VerifierHarness();
    }

    function testAcceptsExactVersionedLowSSignature() public {
        bytes32 digest = keccak256("valid-p256");
        bytes memory signature = p256Signature(KEY1_X, KEY1_Y);
        assertEq(signature.length, 129);
        setP256Expected(digest);

        assertEqBytes4(account.isValidSignature(digest, signature), account.ERC1271_MAGIC_VALUE());
    }

    function testWrapperUsesExactOfficialEip7951InputOrder() public {
        bytes32 digest = 0xbb5a52f42f9c9261ed4361f59422a1e30036e7c32b270c8807a419feca605023;
        uint256 r = 0x2ba3a8be6b94d5ec80a6d9d1190a436effe50d85a1eee859b8cc6af9bd5c2e18;
        uint256 s = 0x4cd60b855d442f5b3c7b11eb6c4e0ae7525fe710fab9aa7c77a67f79e6fadd76;
        bytes memory signature = abi.encodePacked(bytes1(0x01), KEY3_X, KEY3_Y, r, s);
        setP256ExpectedInput(digest, r, s, KEY3_X, KEY3_Y);

        (bool valid, bytes32 signerKeyId) = harness.verify(digest, signature);

        assertTrue(valid);
        assertEq(signerKeyId, P256Verifier.keyId(KEY3_X, KEY3_Y));
    }

    function testRejectsWrongVersionAndLength() public {
        bytes32 digest = keccak256("encoding");
        setP256Expected(digest);
        bytes memory wrongVersion = abi.encodePacked(bytes1(0x02), KEY1_X, KEY1_Y, uint256(1), uint256(1));
        bytes memory shortSignature = abi.encodePacked(bytes1(0x01), KEY1_X, KEY1_Y, uint256(1));

        assertEqBytes4(account.isValidSignature(digest, wrongVersion), account.ERC1271_INVALID_VALUE());
        assertEqBytes4(account.isValidSignature(digest, shortSignature), account.ERC1271_INVALID_VALUE());
    }

    function testRejectsHighSAtApplicationLayer() public {
        bytes32 digest = keccak256("high-s");
        setP256Expected(digest);
        bytes memory highS = p256SignatureWithS(KEY1_X, KEY1_Y, P256_HALF_N + 1);

        assertEqBytes4(account.isValidSignature(digest, highS), account.ERC1271_INVALID_VALUE());
    }

    function testRejectsZeroOutOfRangeAndOffCurveComponents() public {
        bytes32 digest = keccak256("invalid-components");
        setP256Expected(digest);
        bytes memory zeroR = abi.encodePacked(bytes1(0x01), KEY1_X, KEY1_Y, uint256(0), uint256(1));
        bytes memory orderR = abi.encodePacked(bytes1(0x01), KEY1_X, KEY1_Y, P256_N, uint256(1));
        bytes memory offCurve = abi.encodePacked(bytes1(0x01), uint256(1), uint256(1), uint256(1), uint256(1));

        assertEqBytes4(account.isValidSignature(digest, zeroR), account.ERC1271_INVALID_VALUE());
        assertEqBytes4(account.isValidSignature(digest, orderR), account.ERC1271_INVALID_VALUE());
        assertEqBytes4(account.isValidSignature(digest, offCurve), account.ERC1271_INVALID_VALUE());
        assertFalse(harness.validPublicKey(1, 1));
    }

    function testRejectsUnauthorizedPublicKey() public {
        bytes32 digest = keccak256("unauthorized-key");
        setP256Expected(digest);
        assertEqBytes4(account.isValidSignature(digest, p256Signature(KEY2_X, KEY2_Y)), account.ERC1271_INVALID_VALUE());
    }

    function testRejectsEmptyMalformedZeroAndNonOnePrecompileReturns() public {
        bytes32 digest = keccak256("strict-return");
        bytes memory signature = p256Signature(KEY1_X, KEY1_Y);
        setP256Expected(digest);

        for (uint256 mode = 1; mode <= 5; mode++) {
            setP256Mode(mode);
            assertEqBytes4(account.isValidSignature(digest, signature), account.ERC1271_INVALID_VALUE());
        }
    }

    function testMissingPrecompileFailsClosed() public {
        bytes32 digest = keccak256("missing-precompile");
        bytes memory signature = p256Signature(KEY1_X, KEY1_Y);
        vm.etch(P256_PRECOMPILE, hex"");

        (bool valid, bytes32 signerKeyId) = harness.verify(digest, signature);
        assertFalse(valid);
        assertEq(signerKeyId, P256Verifier.keyId(KEY1_X, KEY1_Y));
        assertEqBytes4(account.isValidSignature(digest, signature), account.ERC1271_INVALID_VALUE());
    }

    function testConstructorRejectsOffCurveInitialKey() public {
        vm.expectRevert(BuilderAccount.InvalidDeviceKey.selector);
        new BuilderAccount(1, 1);
    }

    function testFuzzMalformedLengthsNeverValidate(bytes calldata malformed) public {
        vm.assume(malformed.length != 129);
        assertEqBytes4(account.isValidSignature(keccak256("fuzz"), malformed), account.ERC1271_INVALID_VALUE());
    }
}
