// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {EIP712Domain} from "./EIP712Domain.sol";
import {IERC1271} from "./IERC1271.sol";

interface IShotRegistryView {
    function controllerOf(bytes32 shotId) external view returns (address);
    function headOf(bytes32 shotId) external view returns (bytes32);
}

/// @notice Optional aliases, appcoins, and App Store attestations kept outside ShotRegistry.
contract ShotRelations is EIP712Domain {
    bytes4 private constant ERC1271_MAGIC_VALUE = 0x1626ba7e;

    bytes32 public constant CLAIM_HANDLE_TYPEHASH =
        keccak256("ClaimHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)");
    bytes32 public constant RELEASE_HANDLE_TYPEHASH =
        keccak256("ReleaseHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)");
    bytes32 public constant ASSOCIATE_APPCOIN_TYPEHASH =
        keccak256("AssociateAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)");
    bytes32 public constant REMOVE_APPCOIN_TYPEHASH =
        keccak256("RemoveAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)");
    bytes32 public constant ATTEST_APP_STORE_TYPEHASH = keccak256(
        "AttestAppStore(bytes32 shotId,bytes32 bundleIdHash,uint64 storeId,bytes32 evolutionHead,uint64 nonce,uint64 deadline)"
    );

    struct ClaimHandleAction {
        bytes32 shotId;
        bytes32 handleHash;
        uint64 nonce;
        uint64 deadline;
    }

    struct ReleaseHandleAction {
        bytes32 shotId;
        bytes32 handleHash;
        uint64 nonce;
        uint64 deadline;
    }

    struct AssociateAppcoinAction {
        bytes32 shotId;
        uint256 chainId;
        address token;
        uint64 nonce;
        uint64 deadline;
    }

    struct RemoveAppcoinAction {
        bytes32 shotId;
        uint256 chainId;
        address token;
        uint64 nonce;
        uint64 deadline;
    }

    struct AttestAppStoreAction {
        bytes32 shotId;
        bytes32 bundleIdHash;
        uint64 storeId;
        bytes32 evolutionHead;
        uint64 nonce;
        uint64 deadline;
    }

    struct Appcoin {
        uint256 chainId;
        address token;
    }

    struct AppStoreAttestation {
        bytes32 bundleIdHash;
        uint64 storeId;
        bytes32 evolutionHead;
    }

    IShotRegistryView public immutable registry;

    mapping(bytes32 shotId => uint64 nonce) public nonces;
    mapping(bytes32 handleHash => bytes32 shotId) public shotByHandle;
    mapping(bytes32 shotId => bytes32 handleHash) public handleByShot;
    mapping(bytes32 shotId => string handle) private _handleText;
    mapping(bytes32 shotId => Appcoin relation) private _appcoins;
    mapping(bytes32 shotId => AppStoreAttestation attestation) private _appStore;

    error InvalidRegistry();
    error ShotNotFound(bytes32 shotId);
    error InvalidHandle();
    error HandleHashMismatch(bytes32 expected, bytes32 observed);
    error HandleCollision(bytes32 handleHash, bytes32 existingShotId);
    error ShotAlreadyHasHandle(bytes32 shotId, bytes32 handleHash);
    error HandleNotOwned(bytes32 shotId, bytes32 handleHash);
    error InvalidAppcoin();
    error AppcoinMismatch();
    error InvalidAppStoreAttestation();
    error StaleEvolution(bytes32 expected, bytes32 observed);
    error InvalidNonce(uint64 expected, uint64 observed);
    error Expired(uint64 deadline);
    error Unauthorized();

    event HandleClaimed(
        bytes32 indexed shotId, bytes32 indexed handleHash, string handle, uint64 actionNonce, address relayer
    );
    event HandleReleased(
        bytes32 indexed shotId, bytes32 indexed handleHash, string handle, uint64 actionNonce, address relayer
    );
    event AppcoinAssociated(
        bytes32 indexed shotId, uint256 indexed chainId, address indexed token, uint64 actionNonce, address relayer
    );
    event AppcoinRemoved(
        bytes32 indexed shotId, uint256 indexed chainId, address indexed token, uint64 actionNonce, address relayer
    );
    event AppStoreAttested(
        bytes32 indexed shotId,
        bytes32 indexed bundleIdHash,
        uint64 indexed storeId,
        bytes32 evolutionHead,
        uint64 actionNonce,
        address relayer
    );

    constructor(address registryAddress) EIP712Domain("TOHSENO ShotRelations", "1") {
        if (registryAddress == address(0) || registryAddress.code.length == 0) {
            revert InvalidRegistry();
        }
        registry = IShotRegistryView(registryAddress);
    }

    function handleText(bytes32 shotId) external view returns (string memory) {
        return _handleText[shotId];
    }

    function appcoinOf(bytes32 shotId) external view returns (Appcoin memory) {
        return _appcoins[shotId];
    }

    function appStoreAttestationOf(bytes32 shotId) external view returns (AppStoreAttestation memory) {
        return _appStore[shotId];
    }

    function hashClaimHandle(ClaimHandleAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(CLAIM_HANDLE_TYPEHASH, action.shotId, action.handleHash, action.nonce, action.deadline)
            )
        );
    }

    function hashReleaseHandle(ReleaseHandleAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(RELEASE_HANDLE_TYPEHASH, action.shotId, action.handleHash, action.nonce, action.deadline)
            )
        );
    }

    function hashAssociateAppcoin(AssociateAppcoinAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    ASSOCIATE_APPCOIN_TYPEHASH,
                    action.shotId,
                    action.chainId,
                    action.token,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function hashRemoveAppcoin(RemoveAppcoinAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    REMOVE_APPCOIN_TYPEHASH, action.shotId, action.chainId, action.token, action.nonce, action.deadline
                )
            )
        );
    }

    function hashAttestAppStore(AttestAppStoreAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    ATTEST_APP_STORE_TYPEHASH,
                    action.shotId,
                    action.bundleIdHash,
                    action.storeId,
                    action.evolutionHead,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function claimHandle(ClaimHandleAction calldata action, string calldata handle, bytes calldata signature)
        external
    {
        _checkAction(action.shotId, action.nonce, action.deadline);
        if (!_validHandle(handle)) revert InvalidHandle();
        bytes32 observedHash = keccak256(bytes(handle));
        if (action.handleHash != observedHash) {
            revert HandleHashMismatch(action.handleHash, observedHash);
        }

        bytes32 existingForShot = handleByShot[action.shotId];
        if (existingForShot != bytes32(0)) {
            revert ShotAlreadyHasHandle(action.shotId, existingForShot);
        }
        bytes32 existingShot = shotByHandle[action.handleHash];
        if (existingShot != bytes32(0)) {
            revert HandleCollision(action.handleHash, existingShot);
        }
        _requireShotSignature(action.shotId, hashClaimHandle(action), signature);

        nonces[action.shotId] += 1;
        shotByHandle[action.handleHash] = action.shotId;
        handleByShot[action.shotId] = action.handleHash;
        _handleText[action.shotId] = handle;
        emit HandleClaimed(action.shotId, action.handleHash, handle, action.nonce, msg.sender);
    }

    function releaseHandle(ReleaseHandleAction calldata action, bytes calldata signature) external {
        _checkAction(action.shotId, action.nonce, action.deadline);
        if (handleByShot[action.shotId] != action.handleHash || shotByHandle[action.handleHash] != action.shotId) {
            revert HandleNotOwned(action.shotId, action.handleHash);
        }
        _requireShotSignature(action.shotId, hashReleaseHandle(action), signature);

        string memory handle = _handleText[action.shotId];
        nonces[action.shotId] += 1;
        delete shotByHandle[action.handleHash];
        delete handleByShot[action.shotId];
        delete _handleText[action.shotId];
        emit HandleReleased(action.shotId, action.handleHash, handle, action.nonce, msg.sender);
    }

    function associateAppcoin(AssociateAppcoinAction calldata action, bytes calldata signature) external {
        _checkAction(action.shotId, action.nonce, action.deadline);
        if (action.chainId == 0 || action.token == address(0)) revert InvalidAppcoin();
        _requireShotSignature(action.shotId, hashAssociateAppcoin(action), signature);

        nonces[action.shotId] += 1;
        _appcoins[action.shotId] = Appcoin({chainId: action.chainId, token: action.token});
        emit AppcoinAssociated(action.shotId, action.chainId, action.token, action.nonce, msg.sender);
    }

    function removeAppcoin(RemoveAppcoinAction calldata action, bytes calldata signature) external {
        _checkAction(action.shotId, action.nonce, action.deadline);
        Appcoin memory current = _appcoins[action.shotId];
        if (
            current.chainId == 0 || current.token == address(0) || current.chainId != action.chainId
                || current.token != action.token
        ) {
            revert AppcoinMismatch();
        }
        _requireShotSignature(action.shotId, hashRemoveAppcoin(action), signature);

        nonces[action.shotId] += 1;
        delete _appcoins[action.shotId];
        emit AppcoinRemoved(action.shotId, action.chainId, action.token, action.nonce, msg.sender);
    }

    function attestAppStore(AttestAppStoreAction calldata action, bytes calldata signature) external {
        _checkAction(action.shotId, action.nonce, action.deadline);
        if (action.bundleIdHash == bytes32(0) || action.storeId == 0) {
            revert InvalidAppStoreAttestation();
        }
        bytes32 currentHead = registry.headOf(action.shotId);
        if (action.evolutionHead != currentHead) {
            revert StaleEvolution(currentHead, action.evolutionHead);
        }
        _requireShotSignature(action.shotId, hashAttestAppStore(action), signature);

        nonces[action.shotId] += 1;
        _appStore[action.shotId] = AppStoreAttestation({
            bundleIdHash: action.bundleIdHash,
            storeId: action.storeId,
            evolutionHead: action.evolutionHead
        });
        emit AppStoreAttested(
            action.shotId, action.bundleIdHash, action.storeId, action.evolutionHead, action.nonce, msg.sender
        );
    }

    function _checkAction(bytes32 shotId, uint64 nonce, uint64 deadline) private view {
        if (block.timestamp > deadline) revert Expired(deadline);
        if (registry.controllerOf(shotId) == address(0)) revert ShotNotFound(shotId);
        uint64 expectedNonce = nonces[shotId];
        if (nonce != expectedNonce) revert InvalidNonce(expectedNonce, nonce);
    }

    function _requireShotSignature(bytes32 shotId, bytes32 digest, bytes calldata signature) private view {
        address controller = registry.controllerOf(shotId);
        (bool success, bytes memory output) =
            controller.staticcall(abi.encodeCall(IERC1271.isValidSignature, (digest, signature)));
        if (!success || output.length != 32) revert Unauthorized();
        bytes4 result;
        assembly ("memory-safe") {
            result := mload(add(output, 32))
        }
        if (result != ERC1271_MAGIC_VALUE) revert Unauthorized();
    }

    function _validHandle(string calldata handle) private pure returns (bool) {
        bytes calldata value = bytes(handle);
        if (value.length == 0 || value.length > 63) return false;
        if (value[0] == bytes1("-") || value[value.length - 1] == bytes1("-")) return false;
        for (uint256 i = 0; i < value.length; i++) {
            bytes1 character = value[i];
            bool lower = character >= bytes1("a") && character <= bytes1("z");
            bool digit = character >= bytes1("0") && character <= bytes1("9");
            if (!lower && !digit && character != bytes1("-")) return false;
        }
        return true;
    }
}
