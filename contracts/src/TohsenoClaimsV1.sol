// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {EIP712Domain} from "./EIP712Domain.sol";
import {IERC1271} from "./IERC1271.sol";

interface ITohsenoShotRegistry {
    function controllerOf(bytes32 shotId) external view returns (address);
    function headOf(bytes32 shotId) external view returns (bytes32);
}

/// @notice Immutable, non-transferable receipts for encountering Tohseno software.
/// @dev This is an additive product contract. It does not change ShotRegistry or
///      generation-0.8 protocol semantics. Any address may relay an exact action;
///      authorization always comes from the current ERC-1271 account.
contract TohsenoClaimsV1 is EIP712Domain {
    bytes4 private constant ERC1271_MAGIC_VALUE = 0x1626ba7e;

    bytes4 private constant ERC165_INTERFACE_ID = 0x01ffc9a7;
    bytes4 private constant ERC721_INTERFACE_ID = 0x80ac58cd;
    bytes4 private constant ERC721_METADATA_INTERFACE_ID = 0x5b5e139f;
    bytes4 private constant ERC5192_INTERFACE_ID = 0xb45a3c0e;

    bytes32 public constant OPEN_CLAIM_EDITION_TYPEHASH = keccak256(
        "OpenClaimEdition(address shotRegistry,bytes32 shotId,uint64 maxClaims,uint64 closesAt,address controller,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant CLAIM_SOFTWARE_TYPEHASH = keccak256(
        "ClaimSoftware(address shotRegistry,bytes32 shotId,address claimant,bytes32 releaseDigest,bytes32 checkpointDigest,bytes32 gestureCommitment,uint64 nonce,uint64 deadline)"
    );

    string public constant name = "Tohseno Claims";
    string public constant symbol = "TOHSENO-CLAIM";
    string public constant TOKEN_METADATA_BASE = "https://tohseno.com/api/claims/v1/token/";

    struct ClaimEdition {
        bool opened;
        uint64 maxClaims;
        uint64 totalClaims;
        uint64 openedAt;
        uint64 closesAt;
    }

    struct ClaimRecord {
        bytes32 shotId;
        uint64 claimNumber;
        address claimant;
        bytes32 releaseDigest;
        bytes32 checkpointDigest;
        bytes32 gestureCommitment;
    }

    struct OpenClaimEditionAction {
        address shotRegistry;
        bytes32 shotId;
        uint64 maxClaims;
        uint64 closesAt;
        address controller;
        uint64 nonce;
        uint64 deadline;
    }

    struct ClaimSoftwareAction {
        address shotRegistry;
        bytes32 shotId;
        address claimant;
        bytes32 releaseDigest;
        bytes32 checkpointDigest;
        bytes32 gestureCommitment;
        uint64 nonce;
        uint64 deadline;
    }

    ITohsenoShotRegistry public immutable shotRegistry;

    mapping(bytes32 shotId => ClaimEdition) private _editions;
    mapping(bytes32 shotId => mapping(address claimant => uint256 tokenId)) public claimTokenOf;
    mapping(uint256 tokenId => ClaimRecord) private _claims;
    mapping(uint256 tokenId => address owner) private _owners;
    mapping(address owner => uint256 balance) private _balances;

    mapping(address controller => uint64 nonce) public editionNonces;
    mapping(address claimant => uint64 nonce) public claimNonces;

    uint256 public nextTokenId = 1;

    error InvalidShotRegistry();
    error InvalidShotId();
    error ShotNotFound(bytes32 shotId);
    error InvalidController(address expected, address observed);
    error EditionAlreadyOpened(bytes32 shotId);
    error EditionNotOpened(bytes32 shotId);
    error InvalidEditionPolicy();
    error InvalidClaimant();
    error InvalidReleaseDigest();
    error InvalidCheckpointDigest();
    error StaleCheckpoint(bytes32 expected, bytes32 observed);
    error InvalidGestureCommitment();
    error InvalidNonce(uint64 expected, uint64 observed);
    error Expired(uint64 deadline);
    error EditionClosed(bytes32 shotId);
    error SupplyExhausted(bytes32 shotId);
    error AlreadyClaimed(bytes32 shotId, address claimant, uint256 tokenId);
    error Unauthorized();
    error TimestampOverflow();
    error CounterOverflow();
    error TokenNotFound(uint256 tokenId);
    error InvalidOwner();
    error NonTransferable();

    event ClaimEditionOpened(
        bytes32 indexed shotId, address indexed controller, uint64 maxClaims, uint64 opensAt, uint64 closesAt
    );

    event SoftwareClaimed(
        bytes32 indexed shotId,
        address indexed claimant,
        uint256 indexed tokenId,
        uint64 claimNumber,
        bytes32 releaseDigest,
        bytes32 checkpointDigest,
        bytes32 gestureCommitment
    );

    event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
    event Locked(uint256 tokenId);

    constructor(address shotRegistry_) EIP712Domain("TOHSENO Claims", "1") {
        if (shotRegistry_ == address(0) || shotRegistry_.code.length == 0) revert InvalidShotRegistry();
        shotRegistry = ITohsenoShotRegistry(shotRegistry_);
    }

    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        return interfaceId == ERC165_INTERFACE_ID || interfaceId == ERC721_INTERFACE_ID
            || interfaceId == ERC721_METADATA_INTERFACE_ID || interfaceId == ERC5192_INTERFACE_ID;
    }

    function claimEdition(bytes32 shotId) external view returns (ClaimEdition memory) {
        return _editions[shotId];
    }

    function claimRecord(uint256 tokenId) external view returns (ClaimRecord memory) {
        _requireToken(tokenId);
        return _claims[tokenId];
    }

    function ownerOf(uint256 tokenId) public view returns (address) {
        address owner = _owners[tokenId];
        if (owner == address(0)) revert TokenNotFound(tokenId);
        return owner;
    }

    function balanceOf(address owner) external view returns (uint256) {
        if (owner == address(0)) revert InvalidOwner();
        return _balances[owner];
    }

    function tokenURI(uint256 tokenId) external view returns (string memory) {
        _requireToken(tokenId);
        return string.concat(TOKEN_METADATA_BASE, _decimal(tokenId));
    }

    function locked(uint256 tokenId) external view returns (bool) {
        _requireToken(tokenId);
        return true;
    }

    function getApproved(uint256 tokenId) external view returns (address) {
        _requireToken(tokenId);
        return address(0);
    }

    function isApprovedForAll(address, address) external pure returns (bool) {
        return false;
    }

    function approve(address, uint256) external pure {
        revert NonTransferable();
    }

    function setApprovalForAll(address, bool) external pure {
        revert NonTransferable();
    }

    function transferFrom(address, address, uint256) external pure {
        revert NonTransferable();
    }

    function safeTransferFrom(address, address, uint256) external pure {
        revert NonTransferable();
    }

    function safeTransferFrom(address, address, uint256, bytes calldata) external pure {
        revert NonTransferable();
    }

    function editionIsClosed(bytes32 shotId) public view returns (bool) {
        ClaimEdition memory edition = _editions[shotId];
        if (!edition.opened) return false;
        return (edition.maxClaims != 0 && edition.totalClaims >= edition.maxClaims)
            || (edition.closesAt != 0 && block.timestamp >= edition.closesAt);
    }

    function hashOpenClaimEdition(OpenClaimEditionAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    OPEN_CLAIM_EDITION_TYPEHASH,
                    action.shotRegistry,
                    action.shotId,
                    action.maxClaims,
                    action.closesAt,
                    action.controller,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function hashClaimSoftware(ClaimSoftwareAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    CLAIM_SOFTWARE_TYPEHASH,
                    action.shotRegistry,
                    action.shotId,
                    action.claimant,
                    action.releaseDigest,
                    action.checkpointDigest,
                    action.gestureCommitment,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function openClaimEdition(OpenClaimEditionAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        if (action.shotRegistry != address(shotRegistry)) revert InvalidShotRegistry();
        if (action.shotId == bytes32(0)) revert InvalidShotId();
        if (_editions[action.shotId].opened) revert EditionAlreadyOpened(action.shotId);
        if (action.closesAt != 0 && action.closesAt <= block.timestamp) revert InvalidEditionPolicy();

        address observedController = shotRegistry.controllerOf(action.shotId);
        if (observedController == address(0)) revert ShotNotFound(action.shotId);
        if (action.controller != observedController) {
            revert InvalidController(observedController, action.controller);
        }
        uint64 expectedNonce = editionNonces[observedController];
        if (action.nonce != expectedNonce) revert InvalidNonce(expectedNonce, action.nonce);
        _requireContractSignature(observedController, hashOpenClaimEdition(action), signature);

        uint64 openedAt = _timestamp64();
        _editions[action.shotId] = ClaimEdition({
            opened: true,
            maxClaims: action.maxClaims,
            totalClaims: 0,
            openedAt: openedAt,
            closesAt: action.closesAt
        });
        editionNonces[observedController] = _increment(expectedNonce);
        emit ClaimEditionOpened(action.shotId, observedController, action.maxClaims, openedAt, action.closesAt);
    }

    function claimSoftware(ClaimSoftwareAction calldata action, bytes calldata signature)
        external
        returns (uint256 tokenId)
    {
        _checkDeadline(action.deadline);
        if (action.shotRegistry != address(shotRegistry)) revert InvalidShotRegistry();
        if (action.shotId == bytes32(0)) revert InvalidShotId();
        if (action.claimant == address(0) || action.claimant.code.length == 0) revert InvalidClaimant();
        if (action.releaseDigest == bytes32(0)) revert InvalidReleaseDigest();
        if (action.checkpointDigest == bytes32(0)) revert InvalidCheckpointDigest();
        if (action.gestureCommitment == bytes32(0)) revert InvalidGestureCommitment();

        ClaimEdition storage edition = _editions[action.shotId];
        if (!edition.opened) revert EditionNotOpened(action.shotId);
        uint256 priorToken = claimTokenOf[action.shotId][action.claimant];
        if (priorToken != 0) revert AlreadyClaimed(action.shotId, action.claimant, priorToken);
        if (edition.closesAt != 0 && block.timestamp >= edition.closesAt) {
            revert EditionClosed(action.shotId);
        }
        if (edition.maxClaims != 0 && edition.totalClaims >= edition.maxClaims) {
            revert SupplyExhausted(action.shotId);
        }

        bytes32 currentHead = shotRegistry.headOf(action.shotId);
        if (currentHead == bytes32(0)) revert ShotNotFound(action.shotId);
        if (action.checkpointDigest != currentHead) {
            revert StaleCheckpoint(currentHead, action.checkpointDigest);
        }

        uint64 expectedNonce = claimNonces[action.claimant];
        if (action.nonce != expectedNonce) revert InvalidNonce(expectedNonce, action.nonce);
        _requireContractSignature(action.claimant, hashClaimSoftware(action), signature);

        uint64 claimNumber = _increment(edition.totalClaims);
        tokenId = nextTokenId;
        if (tokenId == type(uint256).max) revert CounterOverflow();

        edition.totalClaims = claimNumber;
        nextTokenId = tokenId + 1;
        claimNonces[action.claimant] = _increment(expectedNonce);
        claimTokenOf[action.shotId][action.claimant] = tokenId;
        _claims[tokenId] = ClaimRecord({
            shotId: action.shotId,
            claimNumber: claimNumber,
            claimant: action.claimant,
            releaseDigest: action.releaseDigest,
            checkpointDigest: action.checkpointDigest,
            gestureCommitment: action.gestureCommitment
        });
        _owners[tokenId] = action.claimant;
        _balances[action.claimant] += 1;

        emit Transfer(address(0), action.claimant, tokenId);
        emit Locked(tokenId);
        emit SoftwareClaimed(
            action.shotId,
            action.claimant,
            tokenId,
            claimNumber,
            action.releaseDigest,
            action.checkpointDigest,
            action.gestureCommitment
        );
    }

    function _requireToken(uint256 tokenId) private view {
        if (_owners[tokenId] == address(0)) revert TokenNotFound(tokenId);
    }

    function _checkDeadline(uint64 deadline) private view {
        if (block.timestamp > deadline) revert Expired(deadline);
    }

    function _timestamp64() private view returns (uint64) {
        if (block.timestamp > type(uint64).max) revert TimestampOverflow();
        return uint64(block.timestamp);
    }

    function _increment(uint64 value) private pure returns (uint64) {
        if (value == type(uint64).max) revert CounterOverflow();
        return value + 1;
    }

    function _requireContractSignature(address account, bytes32 digest, bytes calldata signature) private view {
        (bool success, bytes memory output) =
            account.staticcall(abi.encodeCall(IERC1271.isValidSignature, (digest, signature)));
        if (!success || output.length != 32) revert Unauthorized();

        bytes4 result;
        assembly ("memory-safe") {
            result := mload(add(output, 32))
        }
        if (result != ERC1271_MAGIC_VALUE) revert Unauthorized();
    }

    function _decimal(uint256 value) private pure returns (string memory) {
        if (value == 0) return "0";
        uint256 digits;
        uint256 cursor = value;
        while (cursor != 0) {
            digits += 1;
            cursor /= 10;
        }
        bytes memory buffer = new bytes(digits);
        cursor = value;
        while (cursor != 0) {
            digits -= 1;
            buffer[digits] = bytes1(uint8(48 + cursor % 10));
            cursor /= 10;
        }
        return string(buffer);
    }
}
