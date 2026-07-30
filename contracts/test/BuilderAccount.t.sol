// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {BuilderAccountFactory} from "../src/BuilderAccountFactory.sol";
import {IERC1271} from "../src/IERC1271.sol";

contract ERC1271RecoveryAuthority is IERC1271 {
    bytes4 private constant MAGIC = 0x1626ba7e;
    bytes4 private constant INVALID = 0xffffffff;

    bytes32 private _digest;
    bytes32 private _signatureHash;
    uint8 private _mode;

    function authorize(bytes32 digest, bytes calldata signature) external {
        _digest = digest;
        _signatureHash = keccak256(signature);
    }

    function setMode(uint8 mode) external {
        _mode = mode;
    }

    function isValidSignature(bytes32 digest, bytes calldata signature) external view returns (bytes4) {
        uint8 mode = _mode;
        if (mode == 1) return INVALID;
        if (mode == 2) {
            assembly ("memory-safe") {
                revert(0, 0)
            }
        }
        if (mode == 3) {
            assembly ("memory-safe") {
                return(0, 0)
            }
        }
        if (mode == 4) {
            assembly ("memory-safe") {
                mstore(0, MAGIC)
                return(1, 31)
            }
        }
        if (mode == 5) {
            assembly ("memory-safe") {
                mstore(0, MAGIC)
                mstore(32, 0)
                return(0, 64)
            }
        }
        if (digest == _digest && keccak256(signature) == _signatureHash) return MAGIC;
        return INVALID;
    }
}

contract BuilderAccountTest is ProtocolTestBase {
    uint256 private constant RECOVERY_PRIVATE_KEY = 0xa11ce;
    uint256 private constant NEXT_RECOVERY_PRIVATE_KEY = 0xb0b;

    BuilderAccount private account;
    address private recovery;

    function setUp() public {
        installP256Mock();
        account = newAccount(KEY1_X, KEY1_Y);
        recovery = vm.addr(RECOVERY_PRIVATE_KEY);
    }

    function testFactoryPredictionIsRecoveryIndependentAndStableAfterSetup() public {
        BuilderAccountFactory factory = new BuilderAccountFactory();
        bytes32 salt = keccak256("builder");
        address predicted = factory.predictAccount(salt, KEY1_X, KEY1_Y);
        BuilderAccount created = factory.createAccount(salt, KEY1_X, KEY1_Y);
        assertEq(address(created), predicted);
        assertEq(created.recoveryAuthority(), address(0));

        _setRecovery(created, recovery);

        assertEq(address(created), predicted);
        assertEq(factory.predictAccount(salt, KEY1_X, KEY1_Y), predicted);
        assertEq(address(factory.createAccount(salt, KEY1_X, KEY1_Y)), predicted);
        assertEq(created.recoveryAuthority(), recovery);
    }

    function testOneTimeRecoverySetupIsDeviceAuthorized() public {
        _setRecovery(account, recovery);
        assertTrue(account.recoveryConfigured());
        assertEq(account.recoveryAuthority(), recovery);
        assertEq(account.deviceNonce(), uint64(1));

        uint64 deadline = _deadline();
        bytes32 digest = account.hashSetRecovery(vm.addr(NEXT_RECOVERY_PRIVATE_KEY), 1, deadline);
        setP256Expected(digest);
        vm.expectRevert(BuilderAccount.RecoveryAlreadyConfigured.selector);
        account.setRecovery(vm.addr(NEXT_RECOVERY_PRIVATE_KEY), deadline, p256Signature(KEY1_X, KEY1_Y));
    }

    function testUnauthorizedKeyCannotSetRecovery() public {
        uint64 deadline = _deadline();
        bytes32 digest = account.hashSetRecovery(recovery, 0, deadline);
        setP256Expected(digest);

        vm.expectRevert(BuilderAccount.InvalidDeviceSignature.selector);
        account.setRecovery(recovery, deadline, p256Signature(KEY2_X, KEY2_Y));
    }

    function testAuthorizesNarrowlyPermissionedDevice() public {
        _authorize(account, KEY2_X, KEY2_Y, account.PERMISSION_PROTOCOL(), KEY1_X, KEY1_Y);
        bytes32 keyId = account.deviceKeyId(KEY2_X, KEY2_Y);
        assertTrue(account.isAuthorizedKey(keyId));
        assertEq(uint256(account.keyPermissions(keyId)), uint256(account.PERMISSION_PROTOCOL()));
        assertEq(account.activeDeviceCount(), uint64(2));
        assertEq(account.activeAdminCount(), uint64(1));
    }

    function testAuthorizingAdminIncrementsBothCounts() public {
        _authorize(account, KEY2_X, KEY2_Y, account.ALL_PERMISSIONS(), KEY1_X, KEY1_Y);
        assertEq(account.activeDeviceCount(), uint64(2));
        assertEq(account.activeAdminCount(), uint64(2));
    }

    function testProtocolOnlyDeviceCannotManageDevices() public {
        _authorize(account, KEY2_X, KEY2_Y, account.PERMISSION_PROTOCOL(), KEY1_X, KEY1_Y);
        uint64 deadline = _deadline();
        uint64 nonce = account.deviceNonce();
        bytes32 digest = account.hashAuthorizeDevice(KEY3_X, KEY3_Y, account.ALL_PERMISSIONS(), nonce, deadline);
        setP256Expected(digest);

        uint32 permissions = account.ALL_PERMISSIONS();
        vm.expectRevert(BuilderAccount.InvalidDeviceSignature.selector);
        account.authorizeDevice(KEY3_X, KEY3_Y, permissions, deadline, p256Signature(KEY2_X, KEY2_Y));
    }

    function testUnauthorizedDeviceCannotAuthorizeAnother() public {
        uint64 deadline = _deadline();
        bytes32 digest = account.hashAuthorizeDevice(KEY3_X, KEY3_Y, account.ALL_PERMISSIONS(), 0, deadline);
        setP256Expected(digest);

        uint32 permissions = account.ALL_PERMISSIONS();
        vm.expectRevert(BuilderAccount.InvalidDeviceSignature.selector);
        account.authorizeDevice(KEY3_X, KEY3_Y, permissions, deadline, p256Signature(KEY2_X, KEY2_Y));
    }

    function testRevokedDeviceCannotSign() public {
        _authorize(account, KEY2_X, KEY2_Y, account.ALL_PERMISSIONS(), KEY1_X, KEY1_Y);
        bytes32 keyId = account.deviceKeyId(KEY2_X, KEY2_Y);
        uint64 deadline = _deadline();
        uint64 nonce = account.deviceNonce();
        bytes32 revokeDigest = account.hashRevokeDevice(keyId, nonce, deadline);
        setP256Expected(revokeDigest);
        account.revokeDevice(keyId, deadline, p256Signature(KEY1_X, KEY1_Y));

        assertFalse(account.isAuthorizedKey(keyId));
        bytes32 arbitraryDigest = keccak256("after-revocation");
        setP256Expected(arbitraryDigest);
        assertEqBytes4(
            account.isValidSignature(arbitraryDigest, p256Signature(KEY2_X, KEY2_Y)), account.ERC1271_INVALID_VALUE()
        );
    }

    function testCannotRevokeLastDeviceWithoutRecovery() public {
        bytes32 keyId = account.deviceKeyId(KEY1_X, KEY1_Y);
        vm.expectRevert(BuilderAccount.LastDevice.selector);
        account.revokeDevice(keyId, _deadline(), p256Signature(KEY1_X, KEY1_Y));
    }

    function testCannotRevokeLastDeviceEvenWhenRecoveryIsAvailable() public {
        _setRecovery(account, recovery);
        bytes32 keyId = account.deviceKeyId(KEY1_X, KEY1_Y);
        vm.expectRevert(BuilderAccount.LastDevice.selector);
        account.revokeDevice(keyId, _deadline(), p256Signature(KEY1_X, KEY1_Y));
        assertEq(account.activeDeviceCount(), uint64(1));
        assertEq(account.activeAdminCount(), uint64(1));
    }

    function testCannotRevokeLastAdminWhileProtocolOnlyDeviceRemainsWithoutRecovery() public {
        _authorize(account, KEY2_X, KEY2_Y, account.PERMISSION_PROTOCOL(), KEY1_X, KEY1_Y);
        bytes32 adminKeyId = account.deviceKeyId(KEY1_X, KEY1_Y);

        vm.expectRevert(BuilderAccount.LastAdminWithoutRecovery.selector);
        account.revokeDevice(adminKeyId, _deadline(), p256Signature(KEY1_X, KEY1_Y));

        assertEq(account.activeDeviceCount(), uint64(2));
        assertEq(account.activeAdminCount(), uint64(1));
    }

    function testRecoveryAuthorityAllowsLastAdminRevocationButPreservesOneDevice() public {
        _setRecovery(account, recovery);
        _authorize(account, KEY2_X, KEY2_Y, account.PERMISSION_PROTOCOL(), KEY1_X, KEY1_Y);
        bytes32 adminKeyId = account.deviceKeyId(KEY1_X, KEY1_Y);
        uint64 deadline = _deadline();
        bytes32 digest = account.hashRevokeDevice(adminKeyId, account.deviceNonce(), deadline);
        setP256Expected(digest);

        account.revokeDevice(adminKeyId, deadline, p256Signature(KEY1_X, KEY1_Y));

        assertEq(account.activeDeviceCount(), uint64(1));
        assertEq(account.activeAdminCount(), uint64(0));
        assertEq(account.recoveryAuthority(), recovery);
    }

    function testDelayedRecoveryInvalidatesPriorEpochAndRotatesAuthority() public {
        _setRecovery(account, recovery);
        _authorize(account, KEY2_X, KEY2_Y, account.ALL_PERMISSIONS(), KEY1_X, KEY1_Y);
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        uint64 deadline = _deadline();
        bytes32 digest = account.hashInitiateRecovery(KEY3_X, KEY3_Y, nextRecovery, 0, deadline);
        bytes memory signature = recoverySignature(RECOVERY_PRIVATE_KEY, digest);

        bytes32 recoveryId = account.initiateRecovery(KEY3_X, KEY3_Y, nextRecovery, deadline, signature);
        assertEq(recoveryId, digest);
        vm.expectPartialRevert(BuilderAccount.RecoveryDelayActive.selector);
        account.finalizeRecovery(recoveryId);

        vm.warp(block.timestamp + account.RECOVERY_DELAY());
        vm.prank(address(0xbeef));
        account.finalizeRecovery(recoveryId);

        assertEq(account.deviceEpoch(), uint64(2));
        assertEq(account.activeDeviceCount(), uint64(1));
        assertEq(account.activeAdminCount(), uint64(1));
        assertEq(account.recoveryAuthority(), nextRecovery);
        assertFalse(account.isAuthorizedKey(account.deviceKeyId(KEY1_X, KEY1_Y)));
        assertFalse(account.isAuthorizedKey(account.deviceKeyId(KEY2_X, KEY2_Y)));
        assertTrue(account.isAuthorizedKey(account.deviceKeyId(KEY3_X, KEY3_Y)));

        vm.expectRevert(BuilderAccount.RecoveryNotPending.selector);
        account.finalizeRecovery(recoveryId);
    }

    function testWrongRecoveryAuthorityIsRejected() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        bytes32 digest = account.hashInitiateRecovery(KEY2_X, KEY2_Y, recovery, 0, deadline);
        bytes memory wrongSignature = recoverySignature(NEXT_RECOVERY_PRIVATE_KEY, digest);

        vm.expectRevert(BuilderAccount.InvalidRecoverySignature.selector);
        account.initiateRecovery(KEY2_X, KEY2_Y, recovery, deadline, wrongSignature);
    }

    function testRecoveryCannotRemoveRecoveryAuthority() public {
        _setRecovery(account, recovery);
        vm.expectRevert(BuilderAccount.InvalidRecoveryAuthority.selector);
        account.initiateRecovery(KEY2_X, KEY2_Y, address(0), _deadline(), new bytes(65));
    }

    function testERC1271OnlyRecoveryAuthorityCanCompleteDelayedRecovery() public {
        ERC1271RecoveryAuthority contractRecovery = new ERC1271RecoveryAuthority();
        _setRecovery(account, address(contractRecovery));
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        uint64 deadline = _deadline();
        bytes memory signature = hex"cafe";
        bytes32 digest = account.hashInitiateRecovery(KEY2_X, KEY2_Y, nextRecovery, 0, deadline);
        contractRecovery.authorize(digest, signature);

        bytes32 recoveryId = account.initiateRecovery(KEY2_X, KEY2_Y, nextRecovery, deadline, signature);
        vm.warp(block.timestamp + account.RECOVERY_DELAY());
        account.finalizeRecovery(recoveryId);

        assertEq(account.recoveryAuthority(), nextRecovery);
        assertTrue(account.isAuthorizedKey(account.deviceKeyId(KEY2_X, KEY2_Y)));
    }

    function testERC1271RecoveryRequiresExactSuccessfulMagicReturn() public {
        ERC1271RecoveryAuthority contractRecovery = new ERC1271RecoveryAuthority();
        _setRecovery(account, address(contractRecovery));
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        uint64 deadline = _deadline();
        bytes memory signature = hex"cafe";
        bytes32 digest = account.hashInitiateRecovery(KEY2_X, KEY2_Y, nextRecovery, 0, deadline);
        contractRecovery.authorize(digest, signature);

        for (uint8 mode = 1; mode <= 5; ++mode) {
            contractRecovery.setMode(mode);
            vm.expectRevert(BuilderAccount.InvalidRecoverySignature.selector);
            account.initiateRecovery(KEY2_X, KEY2_Y, nextRecovery, deadline, signature);
            assertEq(account.recoveryNonce(), uint64(0));
        }
    }

    function testActiveAdminCanCancelPendingRecovery() public {
        _setRecovery(account, recovery);
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        uint64 deadline = _deadline();
        bytes32 recoveryId = _initiateEoaRecovery(KEY2_X, KEY2_Y, nextRecovery, deadline);
        bytes32 cancelDigest = account.hashCancelRecovery(recoveryId, account.deviceNonce(), deadline);
        setP256Expected(cancelDigest);

        account.cancelRecovery(recoveryId, deadline, p256Signature(KEY1_X, KEY1_Y));

        vm.warp(block.timestamp + account.RECOVERY_DELAY());
        vm.expectRevert(BuilderAccount.RecoveryNotPending.selector);
        account.finalizeRecovery(recoveryId);
    }

    function testPendingRecoveryCannotBeSilentlyReplaced() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        bytes32 recoveryId = _initiateEoaRecovery(KEY2_X, KEY2_Y, vm.addr(NEXT_RECOVERY_PRIVATE_KEY), deadline);
        bytes32 digest = account.hashInitiateRecovery(KEY3_X, KEY3_Y, recovery, 1, deadline);

        vm.expectRevert(abi.encodeWithSelector(BuilderAccount.RecoveryAlreadyPending.selector, recoveryId));
        account.initiateRecovery(KEY3_X, KEY3_Y, recovery, deadline, recoverySignature(RECOVERY_PRIVATE_KEY, digest));
    }

    function testProtocolOnlyDeviceCannotCancelRecovery() public {
        _setRecovery(account, recovery);
        _authorize(account, KEY2_X, KEY2_Y, account.PERMISSION_PROTOCOL(), KEY1_X, KEY1_Y);
        uint64 deadline = _deadline();
        bytes32 recoveryId = _initiateEoaRecovery(KEY3_X, KEY3_Y, vm.addr(NEXT_RECOVERY_PRIVATE_KEY), deadline);
        bytes32 cancelDigest = account.hashCancelRecovery(recoveryId, account.deviceNonce(), deadline);
        setP256Expected(cancelDigest);

        vm.expectRevert(BuilderAccount.InvalidDeviceSignature.selector);
        account.cancelRecovery(recoveryId, deadline, p256Signature(KEY2_X, KEY2_Y));
    }

    function testAdminCanCancelAfterMaturityUntilFinalizationLands() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        bytes32 recoveryId = _initiateEoaRecovery(KEY2_X, KEY2_Y, vm.addr(NEXT_RECOVERY_PRIVATE_KEY), deadline);
        vm.warp(block.timestamp + account.RECOVERY_DELAY());
        uint64 cancelDeadline = _deadline();
        bytes32 cancelDigest = account.hashCancelRecovery(recoveryId, account.deviceNonce(), cancelDeadline);
        setP256Expected(cancelDigest);

        account.cancelRecovery(recoveryId, cancelDeadline, p256Signature(KEY1_X, KEY1_Y));

        vm.expectRevert(BuilderAccount.RecoveryNotPending.selector);
        account.finalizeRecovery(recoveryId);
    }

    function testWrongRecoveryIdDoesNotCancelOrFinalize() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        bytes32 recoveryId = _initiateEoaRecovery(KEY2_X, KEY2_Y, vm.addr(NEXT_RECOVERY_PRIVATE_KEY), deadline);
        bytes32 wrongId = keccak256("wrong recovery");

        vm.expectRevert(abi.encodeWithSelector(BuilderAccount.RecoveryIdMismatch.selector, recoveryId, wrongId));
        account.cancelRecovery(wrongId, deadline, p256Signature(KEY1_X, KEY1_Y));
        vm.warp(block.timestamp + account.RECOVERY_DELAY());
        vm.expectRevert(abi.encodeWithSelector(BuilderAccount.RecoveryIdMismatch.selector, recoveryId, wrongId));
        account.finalizeRecovery(wrongId);
    }

    function testExpiredAndInvalidRecoveryInitiationsFailBeforeMutation() public {
        _setRecovery(account, recovery);
        vm.warp(100);
        vm.expectPartialRevert(BuilderAccount.Expired.selector);
        account.initiateRecovery(KEY2_X, KEY2_Y, recovery, 99, new bytes(65));
        vm.expectRevert(BuilderAccount.InvalidDeviceKey.selector);
        account.initiateRecovery(1, 1, recovery, _deadline(), new bytes(65));
        assertEq(account.recoveryNonce(), uint64(0));
    }

    function testDeviceAdminCanCorrectRecoveryAndInvalidatesPendingRecovery() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        bytes32 recoveryId = _initiateEoaRecovery(KEY2_X, KEY2_Y, vm.addr(NEXT_RECOVERY_PRIVATE_KEY), deadline);
        address corrected = vm.addr(0xc0ffee);
        bytes32 changeDigest = account.hashChangeRecovery(corrected, account.deviceNonce(), deadline);
        setP256Expected(changeDigest);

        account.changeRecovery(corrected, deadline, p256Signature(KEY1_X, KEY1_Y));

        assertEq(account.recoveryAuthority(), corrected);
        assertEq(account.recoveryNonce(), uint64(2));
        vm.expectRevert(BuilderAccount.RecoveryNotPending.selector);
        account.finalizeRecovery(recoveryId);
    }

    function testChangedRecoveryRejectsOldAuthorityPresignature() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        bytes32 oldDigest = account.hashInitiateRecovery(KEY2_X, KEY2_Y, nextRecovery, 0, deadline);
        bytes memory oldSignature = recoverySignature(RECOVERY_PRIVATE_KEY, oldDigest);
        address corrected = vm.addr(0xc0ffee);
        bytes32 changeDigest = account.hashChangeRecovery(corrected, account.deviceNonce(), deadline);
        setP256Expected(changeDigest);
        account.changeRecovery(corrected, deadline, p256Signature(KEY1_X, KEY1_Y));

        vm.expectRevert(BuilderAccount.InvalidRecoverySignature.selector);
        account.initiateRecovery(KEY2_X, KEY2_Y, nextRecovery, deadline, oldSignature);
    }

    function testRecoveryActionsAreChainScoped() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        bytes32 digest = account.hashInitiateRecovery(KEY2_X, KEY2_Y, nextRecovery, 0, deadline);
        bytes memory signature = recoverySignature(RECOVERY_PRIVATE_KEY, digest);

        vm.chainId(block.chainid + 1);
        vm.expectRevert(BuilderAccount.InvalidRecoverySignature.selector);
        account.initiateRecovery(KEY2_X, KEY2_Y, nextRecovery, deadline, signature);
    }

    function testDeviceAdminRecoveryActionsAreChainScoped() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        bytes32 digest = account.hashChangeRecovery(nextRecovery, account.deviceNonce(), deadline);
        setP256Expected(digest);

        vm.chainId(block.chainid + 1);
        vm.expectRevert(BuilderAccount.InvalidDeviceSignature.selector);
        account.changeRecovery(nextRecovery, deadline, p256Signature(KEY1_X, KEY1_Y));
    }

    function testSelfCannotBeConfiguredAsRecoveryAuthority() public {
        uint64 deadline = _deadline();
        bytes32 digest = account.hashSetRecovery(address(account), 0, deadline);
        setP256Expected(digest);
        vm.expectRevert(BuilderAccount.InvalidRecoveryAuthority.selector);
        account.setRecovery(address(account), deadline, p256Signature(KEY1_X, KEY1_Y));
    }

    function testWrongAccountDomainIsRejected() public {
        BuilderAccount other = newAccount(KEY1_X, KEY1_Y);
        uint64 deadline = _deadline();
        bytes32 wrongDigest = other.hashAuthorizeDevice(KEY2_X, KEY2_Y, other.ALL_PERMISSIONS(), 0, deadline);
        setP256Expected(wrongDigest);

        uint32 permissions = account.ALL_PERMISSIONS();
        vm.expectRevert(BuilderAccount.InvalidDeviceSignature.selector);
        account.authorizeDevice(KEY2_X, KEY2_Y, permissions, deadline, p256Signature(KEY1_X, KEY1_Y));
    }

    function testExpiredDeviceActionIsRejected() public {
        vm.warp(100);
        uint32 permissions = account.ALL_PERMISSIONS();
        vm.expectPartialRevert(BuilderAccount.Expired.selector);
        account.authorizeDevice(KEY2_X, KEY2_Y, permissions, 99, p256Signature(KEY1_X, KEY1_Y));
    }

    function testFuzzInvalidPermissionBitsAreRejected(uint32 permissions) public {
        vm.assume(permissions == 0 || (permissions & ~account.ALL_PERMISSIONS()) != 0);
        vm.expectPartialRevert(BuilderAccount.InvalidPermissions.selector);
        account.authorizeDevice(KEY2_X, KEY2_Y, permissions, _deadline(), p256Signature(KEY1_X, KEY1_Y));
    }

    function _setRecovery(BuilderAccount target, address recoveryAddress) private {
        uint64 deadline = _deadline();
        uint64 nonce = target.deviceNonce();
        bytes32 digest = target.hashSetRecovery(recoveryAddress, nonce, deadline);
        setP256Expected(digest);
        target.setRecovery(recoveryAddress, deadline, p256Signature(KEY1_X, KEY1_Y));
    }

    function _authorize(
        BuilderAccount target,
        uint256 newX,
        uint256 newY,
        uint32 permissions,
        uint256 signerX,
        uint256 signerY
    ) private {
        uint64 deadline = _deadline();
        uint64 nonce = target.deviceNonce();
        bytes32 digest = target.hashAuthorizeDevice(newX, newY, permissions, nonce, deadline);
        setP256Expected(digest);
        target.authorizeDevice(newX, newY, permissions, deadline, p256Signature(signerX, signerY));
    }

    function _initiateEoaRecovery(uint256 newX, uint256 newY, address newRecovery, uint64 deadline)
        private
        returns (bytes32)
    {
        uint64 nonce = account.recoveryNonce();
        bytes32 digest = account.hashInitiateRecovery(newX, newY, newRecovery, nonce, deadline);
        bytes memory signature = recoverySignature(RECOVERY_PRIVATE_KEY, digest);
        return account.initiateRecovery(newX, newY, newRecovery, deadline, signature);
    }

    function _deadline() private view returns (uint64) {
        return uint64(block.timestamp + 1 days);
    }
}
