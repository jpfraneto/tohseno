// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";

contract BuilderAccountInvariantTest is ProtocolTestBase {
    uint256 private constant RECOVERY_PRIVATE_KEY = 0xa11ce;

    BuilderAccount private account;
    address private recovery;

    function setUp() public {
        installP256Mock();
        account = newAccount(KEY1_X, KEY1_Y);
        recovery = vm.addr(RECOVERY_PRIVATE_KEY);
    }

    function testFuzzDeviceCountsMatchTheCurrentEpoch(bytes calldata operations) public {
        uint256 operationCount = operations.length;
        if (operationCount > 32) operationCount = 32;

        _assertCounts();
        for (uint256 index; index < operationCount; ++index) {
            _apply(operations[index]);
            _assertCounts();
        }
    }

    function _apply(bytes1 operation) private {
        uint8 value = uint8(operation);
        uint8 keyIndex = (value >> 2) % 3;
        (uint256 x, uint256 y) = _coordinates(keyIndex);
        uint8 action = value & 3;

        if (action == 0) {
            _authorizeIfPossible(x, y, (value & 0x20) == 0);
        } else if (action == 1) {
            _revokeIfPossible(x, y);
        } else if (action == 2) {
            _configureRecoveryIfPossible();
        } else {
            _recoverIfPossible(x, y);
        }
    }

    function _authorizeIfPossible(uint256 x, uint256 y, bool protocolOnly) private {
        bytes32 keyId = account.deviceKeyId(x, y);
        if (account.isAuthorizedKey(keyId)) return;
        (bool found, uint256 signerX, uint256 signerY) = _adminSigner();
        if (!found) return;

        uint32 permissions = protocolOnly ? account.PERMISSION_PROTOCOL() : account.ALL_PERMISSIONS();
        uint64 deadline = _deadline();
        bytes32 digest = account.hashAuthorizeDevice(x, y, permissions, account.deviceNonce(), deadline);
        setP256Expected(digest);
        account.authorizeDevice(x, y, permissions, deadline, p256Signature(signerX, signerY));
    }

    function _revokeIfPossible(uint256 x, uint256 y) private {
        bytes32 keyId = account.deviceKeyId(x, y);
        if (!account.isAuthorizedKey(keyId) || account.activeDeviceCount() == 1) return;
        uint32 permissions = account.keyPermissions(keyId);
        if (
            (permissions & account.PERMISSION_DEVICE_ADMIN()) != 0 && account.activeAdminCount() == 1
                && account.recoveryAuthority() == address(0)
        ) return;
        (bool found, uint256 signerX, uint256 signerY) = _adminSigner();
        if (!found) return;

        uint64 deadline = _deadline();
        bytes32 digest = account.hashRevokeDevice(keyId, account.deviceNonce(), deadline);
        setP256Expected(digest);
        account.revokeDevice(keyId, deadline, p256Signature(signerX, signerY));
    }

    function _configureRecoveryIfPossible() private {
        if (account.recoveryAuthority() != address(0)) return;
        (bool found, uint256 signerX, uint256 signerY) = _adminSigner();
        if (!found) return;

        uint64 deadline = _deadline();
        bytes32 digest = account.hashSetRecovery(recovery, account.deviceNonce(), deadline);
        setP256Expected(digest);
        account.setRecovery(recovery, deadline, p256Signature(signerX, signerY));
    }

    function _recoverIfPossible(uint256 x, uint256 y) private {
        if (account.recoveryAuthority() == address(0)) return;

        uint64 deadline = _deadline();
        bytes32 digest = account.hashRecovery(x, y, recovery, account.recoveryNonce(), deadline);
        account.recover(x, y, recovery, deadline, recoverySignature(RECOVERY_PRIVATE_KEY, digest));
    }

    function _assertCounts() private view {
        uint64 deviceCount;
        uint64 adminCount;
        for (uint8 index; index < 3; ++index) {
            (uint256 x, uint256 y) = _coordinates(index);
            uint32 permissions = account.keyPermissions(account.deviceKeyId(x, y));
            if (permissions == 0) continue;
            deviceCount += 1;
            if ((permissions & account.PERMISSION_DEVICE_ADMIN()) != 0) adminCount += 1;
        }

        assertEq(account.activeDeviceCount(), deviceCount);
        assertEq(account.activeAdminCount(), adminCount);
        assertTrue(deviceCount >= 1);
        assertTrue(adminCount >= 1 || account.recoveryAuthority() != address(0));
    }

    function _adminSigner() private view returns (bool, uint256, uint256) {
        for (uint8 index; index < 3; ++index) {
            (uint256 x, uint256 y) = _coordinates(index);
            bytes32 keyId = account.deviceKeyId(x, y);
            if (account.hasPermission(keyId, account.PERMISSION_DEVICE_ADMIN())) {
                return (true, x, y);
            }
        }
        return (false, 0, 0);
    }

    function _coordinates(uint8 index) private pure returns (uint256, uint256) {
        if (index == 0) return (KEY1_X, KEY1_Y);
        if (index == 1) return (KEY2_X, KEY2_Y);
        return (KEY3_X, KEY3_Y);
    }

    function _deadline() private view returns (uint64) {
        return uint64(block.timestamp + 1 days);
    }
}
