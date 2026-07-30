// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {EIP712Domain} from "./EIP712Domain.sol";
import {IERC1271} from "./IERC1271.sol";
import {P256Verifier} from "./P256Verifier.sol";

/// @notice Non-upgradeable BuilderID account controlled by replaceable P-256 device keys.
contract BuilderAccount is EIP712Domain, IERC1271 {
    bytes4 public constant ERC1271_MAGIC_VALUE = 0x1626ba7e;
    bytes4 public constant ERC1271_INVALID_VALUE = 0xffffffff;

    uint32 public constant PERMISSION_PROTOCOL = 1 << 0;
    uint32 public constant PERMISSION_DEVICE_ADMIN = 1 << 1;
    uint32 public constant ALL_PERMISSIONS = PERMISSION_PROTOCOL | PERMISSION_DEVICE_ADMIN;

    bytes32 public constant AUTHORIZE_DEVICE_TYPEHASH = keccak256(
        "AuthorizeDevice(address account,bytes32 keyId,uint256 x,uint256 y,uint32 permissions,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant REVOKE_DEVICE_TYPEHASH =
        keccak256("RevokeDevice(address account,bytes32 keyId,uint64 nonce,uint64 deadline)");
    bytes32 public constant SET_RECOVERY_TYPEHASH =
        keccak256("SetRecovery(address account,address recovery,uint64 nonce,uint64 deadline)");
    bytes32 public constant RECOVER_ACCOUNT_TYPEHASH = keccak256(
        "RecoverAccount(address account,address currentRecovery,address newRecovery,bytes32 newKeyId,uint256 newX,uint256 newY,uint64 nonce,uint64 deadline)"
    );

    uint256 private constant SECP256K1_HALF_ORDER = 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;

    mapping(bytes32 keyId => uint64 epoch) private _keyEpoch;
    mapping(bytes32 keyId => uint32 permissions) private _keyPermissions;

    uint64 public deviceEpoch;
    uint64 public deviceNonce;
    uint64 public recoveryNonce;
    uint64 public activeDeviceCount;
    uint64 public activeAdminCount;
    address public recoveryAuthority;
    bool public recoveryConfigured;

    error Expired(uint64 deadline);
    error InvalidDeviceKey();
    error DeviceAlreadyAuthorized(bytes32 keyId);
    error DeviceNotAuthorized(bytes32 keyId);
    error InvalidPermissions(uint32 permissions);
    error InvalidDeviceSignature();
    error LastDevice();
    error LastAdminWithoutRecovery();
    error RecoveryAlreadyConfigured();
    error InvalidRecoveryAuthority();
    error RecoveryDisabled();
    error InvalidRecoverySignature();

    event DeviceAuthorized(bytes32 indexed keyId, uint256 x, uint256 y, uint32 permissions, uint64 indexed epoch);
    event DeviceRevoked(bytes32 indexed keyId, uint64 indexed epoch);
    event RecoveryConfigured(address indexed recoveryAuthority);
    event AccountRecovered(
        address indexed previousRecovery, address indexed newRecovery, bytes32 indexed replacementKeyId, uint64 newEpoch
    );

    constructor(uint256 initialX, uint256 initialY) EIP712Domain("TOHSENO BuilderAccount", "1") {
        if (!P256Verifier.validPublicKey(initialX, initialY)) revert InvalidDeviceKey();
        deviceEpoch = 1;
        activeDeviceCount = 1;
        activeAdminCount = 1;
        bytes32 initialKeyId = P256Verifier.keyId(initialX, initialY);
        _keyEpoch[initialKeyId] = deviceEpoch;
        _keyPermissions[initialKeyId] = ALL_PERMISSIONS;
        emit DeviceAuthorized(initialKeyId, initialX, initialY, ALL_PERMISSIONS, deviceEpoch);
    }

    function deviceKeyId(uint256 x, uint256 y) public pure returns (bytes32) {
        return P256Verifier.keyId(x, y);
    }

    function isAuthorizedKey(bytes32 keyId) public view returns (bool) {
        return _keyEpoch[keyId] == deviceEpoch && _keyPermissions[keyId] != 0;
    }

    function keyPermissions(bytes32 keyId) public view returns (uint32) {
        if (_keyEpoch[keyId] != deviceEpoch) return 0;
        return _keyPermissions[keyId];
    }

    function hasPermission(bytes32 keyId, uint32 permission) public view returns (bool) {
        return isAuthorizedKey(keyId) && (_keyPermissions[keyId] & permission) == permission;
    }

    function isValidSignature(bytes32 digest, bytes calldata signature) external view returns (bytes4) {
        (bool valid, bytes32 signerKeyId) = P256Verifier.verify(digest, signature);
        if (valid && hasPermission(signerKeyId, PERMISSION_PROTOCOL)) {
            return ERC1271_MAGIC_VALUE;
        }
        return ERC1271_INVALID_VALUE;
    }

    function hashAuthorizeDevice(uint256 x, uint256 y, uint32 permissions, uint64 nonce, uint64 deadline)
        public
        view
        returns (bytes32)
    {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    AUTHORIZE_DEVICE_TYPEHASH,
                    address(this),
                    P256Verifier.keyId(x, y),
                    x,
                    y,
                    permissions,
                    nonce,
                    deadline
                )
            )
        );
    }

    function hashRevokeDevice(bytes32 keyId, uint64 nonce, uint64 deadline) public view returns (bytes32) {
        return _hashTypedData(keccak256(abi.encode(REVOKE_DEVICE_TYPEHASH, address(this), keyId, nonce, deadline)));
    }

    function hashSetRecovery(address recovery, uint64 nonce, uint64 deadline) public view returns (bytes32) {
        return _hashTypedData(keccak256(abi.encode(SET_RECOVERY_TYPEHASH, address(this), recovery, nonce, deadline)));
    }

    function hashRecovery(uint256 newX, uint256 newY, address newRecovery, uint64 nonce, uint64 deadline)
        public
        view
        returns (bytes32)
    {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    RECOVER_ACCOUNT_TYPEHASH,
                    address(this),
                    recoveryAuthority,
                    newRecovery,
                    P256Verifier.keyId(newX, newY),
                    newX,
                    newY,
                    nonce,
                    deadline
                )
            )
        );
    }

    function authorizeDevice(
        uint256 x,
        uint256 y,
        uint32 permissions,
        uint64 deadline,
        bytes calldata authorizingSignature
    ) external {
        _checkDeadline(deadline);
        if (!P256Verifier.validPublicKey(x, y)) revert InvalidDeviceKey();
        _checkPermissions(permissions);
        bytes32 newKeyId = P256Verifier.keyId(x, y);
        if (isAuthorizedKey(newKeyId)) revert DeviceAlreadyAuthorized(newKeyId);

        uint64 nonce = deviceNonce;
        bytes32 digest = hashAuthorizeDevice(x, y, permissions, nonce, deadline);
        if (!_validAuthorizedDeviceSignature(digest, authorizingSignature, PERMISSION_DEVICE_ADMIN)) {
            revert InvalidDeviceSignature();
        }

        deviceNonce = nonce + 1;
        activeDeviceCount += 1;
        if ((permissions & PERMISSION_DEVICE_ADMIN) != 0) activeAdminCount += 1;
        _keyEpoch[newKeyId] = deviceEpoch;
        _keyPermissions[newKeyId] = permissions;
        emit DeviceAuthorized(newKeyId, x, y, permissions, deviceEpoch);
    }

    function revokeDevice(bytes32 keyId, uint64 deadline, bytes calldata authorizingSignature) external {
        _checkDeadline(deadline);
        if (!isAuthorizedKey(keyId)) revert DeviceNotAuthorized(keyId);
        if (activeDeviceCount == 1) revert LastDevice();
        uint32 permissions = _keyPermissions[keyId];
        bool revokesAdmin = (permissions & PERMISSION_DEVICE_ADMIN) != 0;
        if (revokesAdmin && activeAdminCount == 1 && recoveryAuthority == address(0)) {
            revert LastAdminWithoutRecovery();
        }

        uint64 nonce = deviceNonce;
        bytes32 digest = hashRevokeDevice(keyId, nonce, deadline);
        if (!_validAuthorizedDeviceSignature(digest, authorizingSignature, PERMISSION_DEVICE_ADMIN)) {
            revert InvalidDeviceSignature();
        }

        deviceNonce = nonce + 1;
        activeDeviceCount -= 1;
        if (revokesAdmin) activeAdminCount -= 1;
        _keyEpoch[keyId] = 0;
        _keyPermissions[keyId] = 0;
        emit DeviceRevoked(keyId, deviceEpoch);
    }

    /// @notice Secures recovery after account prediction/deployment without changing BuilderID.
    function setRecovery(address recovery, uint64 deadline, bytes calldata authorizingSignature) external {
        _checkDeadline(deadline);
        if (recoveryConfigured) revert RecoveryAlreadyConfigured();
        if (recovery == address(0)) revert InvalidRecoveryAuthority();

        uint64 nonce = deviceNonce;
        bytes32 digest = hashSetRecovery(recovery, nonce, deadline);
        if (!_validAuthorizedDeviceSignature(digest, authorizingSignature, PERMISSION_DEVICE_ADMIN)) {
            revert InvalidDeviceSignature();
        }

        deviceNonce = nonce + 1;
        recoveryConfigured = true;
        recoveryAuthority = recovery;
        emit RecoveryConfigured(recovery);
    }

    /// @notice Replaces the complete device set with one new key and rotates recovery.
    function recover(uint256 newX, uint256 newY, address newRecovery, uint64 deadline, bytes calldata recoverySignature)
        external
    {
        _checkDeadline(deadline);
        address currentRecovery = recoveryAuthority;
        if (currentRecovery == address(0)) revert RecoveryDisabled();
        if (newRecovery == address(0)) revert InvalidRecoveryAuthority();
        if (!P256Verifier.validPublicKey(newX, newY)) revert InvalidDeviceKey();

        uint64 nonce = recoveryNonce;
        bytes32 digest = hashRecovery(newX, newY, newRecovery, nonce, deadline);
        if (_recoverSigner(digest, recoverySignature) != currentRecovery) {
            revert InvalidRecoverySignature();
        }

        recoveryNonce = nonce + 1;
        deviceEpoch += 1;
        activeDeviceCount = 1;
        activeAdminCount = 1;
        recoveryAuthority = newRecovery;
        bytes32 replacementKeyId = P256Verifier.keyId(newX, newY);
        _keyEpoch[replacementKeyId] = deviceEpoch;
        _keyPermissions[replacementKeyId] = ALL_PERMISSIONS;
        emit AccountRecovered(currentRecovery, newRecovery, replacementKeyId, deviceEpoch);
        emit DeviceAuthorized(replacementKeyId, newX, newY, ALL_PERMISSIONS, deviceEpoch);
    }

    function _validAuthorizedDeviceSignature(bytes32 digest, bytes calldata signature, uint32 requiredPermission)
        private
        view
        returns (bool)
    {
        (bool valid, bytes32 signerKeyId) = P256Verifier.verify(digest, signature);
        return valid && hasPermission(signerKeyId, requiredPermission);
    }

    function _recoverSigner(bytes32 digest, bytes calldata signature) private pure returns (address) {
        if (signature.length != 65) return address(0);
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly ("memory-safe") {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 32))
            v := byte(0, calldataload(add(signature.offset, 64)))
        }
        if (uint256(s) > SECP256K1_HALF_ORDER || (v != 27 && v != 28)) return address(0);
        return ecrecover(digest, v, r, s);
    }

    function _checkDeadline(uint64 deadline) private view {
        if (block.timestamp > deadline) revert Expired(deadline);
    }

    function _checkPermissions(uint32 permissions) private pure {
        if (permissions == 0 || (permissions & ~ALL_PERMISSIONS) != 0) {
            revert InvalidPermissions(permissions);
        }
    }
}
