// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {BuilderAccountFactory} from "../src/BuilderAccountFactory.sol";

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

    function testRecoveryInvalidatesPriorEpochAndRotatesAuthority() public {
        _setRecovery(account, recovery);
        _authorize(account, KEY2_X, KEY2_Y, account.ALL_PERMISSIONS(), KEY1_X, KEY1_Y);
        address nextRecovery = vm.addr(NEXT_RECOVERY_PRIVATE_KEY);
        uint64 deadline = _deadline();
        bytes32 digest = account.hashRecovery(KEY3_X, KEY3_Y, nextRecovery, 0, deadline);
        bytes memory signature = recoverySignature(RECOVERY_PRIVATE_KEY, digest);

        account.recover(KEY3_X, KEY3_Y, nextRecovery, deadline, signature);

        assertEq(account.deviceEpoch(), uint64(2));
        assertEq(account.activeDeviceCount(), uint64(1));
        assertEq(account.activeAdminCount(), uint64(1));
        assertEq(account.recoveryAuthority(), nextRecovery);
        assertFalse(account.isAuthorizedKey(account.deviceKeyId(KEY1_X, KEY1_Y)));
        assertFalse(account.isAuthorizedKey(account.deviceKeyId(KEY2_X, KEY2_Y)));
        assertTrue(account.isAuthorizedKey(account.deviceKeyId(KEY3_X, KEY3_Y)));

        vm.expectRevert(BuilderAccount.InvalidRecoverySignature.selector);
        account.recover(KEY1_X, KEY1_Y, recovery, deadline, signature);
    }

    function testWrongRecoveryAuthorityIsRejected() public {
        _setRecovery(account, recovery);
        uint64 deadline = _deadline();
        bytes32 digest = account.hashRecovery(KEY2_X, KEY2_Y, recovery, 0, deadline);
        bytes memory wrongSignature = recoverySignature(NEXT_RECOVERY_PRIVATE_KEY, digest);

        vm.expectRevert(BuilderAccount.InvalidRecoverySignature.selector);
        account.recover(KEY2_X, KEY2_Y, recovery, deadline, wrongSignature);
    }

    function testRecoveryCannotRemoveRecoveryAuthority() public {
        _setRecovery(account, recovery);
        vm.expectRevert(BuilderAccount.InvalidRecoveryAuthority.selector);
        account.recover(KEY2_X, KEY2_Y, address(0), _deadline(), new bytes(65));
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

    function _deadline() private view returns (uint64) {
        return uint64(block.timestamp + 1 days);
    }
}
