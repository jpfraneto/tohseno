// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {EIP712Domain} from "./EIP712Domain.sol";
import {IERC1271} from "./IERC1271.sol";

/// @notice Neutral public witness for TOHSENO Shot controllers and lineage checkpoints.
/// @dev Commits are permissionless. Reveals and later actions are signature-authenticated,
///      relayer-agnostic, and never authorize `msg.sender`.
contract ShotRegistry is EIP712Domain {
    bytes4 private constant ERC1271_MAGIC_VALUE = 0x1626ba7e;
    bytes3 private constant EIP7702_DELEGATION_PREFIX = 0xef0100;

    uint64 public constant MIN_COMMIT_AGE = 60;
    uint64 public constant MAX_COMMIT_AGE = 24 hours;

    bytes32 public constant REGISTRATION_COMMITMENT_TYPEHASH = keccak256(
        "ShotRegistrationCommitment(address controller,bytes32 shotId,bytes32 salt,address registry,uint256 chainId,uint64 deadline)"
    );
    bytes32 public constant REGISTER_SHOT_TYPEHASH = keccak256(
        "RegisterShot(bytes32 shotId,address controller,bytes32 head,bytes32 salt,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant APPEND_CHECKPOINT_TYPEHASH = keccak256(
        "AppendCheckpoint(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant TRANSFER_SHOT_TYPEHASH = keccak256(
        "TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 checkpointSequence,uint64 nonce,uint64 deadline)"
    );

    struct Shot {
        address controller;
        bytes32 head;
        uint64 checkpointSequence;
        uint64 nonce;
    }

    struct RegistrationCommitment {
        uint64 committedAt;
        bool exists;
    }

    struct RegisterShotAction {
        bytes32 shotId;
        address controller;
        bytes32 head;
        bytes32 salt;
        uint64 nonce;
        uint64 deadline;
    }

    struct AppendCheckpointAction {
        bytes32 shotId;
        bytes32 previousHead;
        bytes32 newHead;
        uint64 checkpointSequence;
        uint64 nonce;
        uint64 deadline;
    }

    struct TransferShotAction {
        bytes32 shotId;
        address currentController;
        address newController;
        bytes32 currentHead;
        uint64 checkpointSequence;
        uint64 nonce;
        uint64 deadline;
    }

    mapping(bytes32 shotId => Shot) private _shots;
    mapping(bytes32 commitment => RegistrationCommitment) private _commitments;
    mapping(address controller => uint64 nonce) public registrationNonces;

    error InvalidShotId();
    error InvalidHead();
    error InvalidController(address controller);
    error DelegatedEOAController(address controller);
    error InvalidCommitment();
    error CommitmentNotFound(bytes32 commitment);
    error CommitmentTooYoung(uint256 availableAt);
    error CommitmentExpired(uint256 expiredAt);
    error ShotAlreadyExists(bytes32 shotId);
    error ShotNotFound(bytes32 shotId);
    error InvalidCheckpointSequence(uint64 expected, uint64 observed);
    error StaleHead(bytes32 expected, bytes32 observed);
    error InvalidNonce(uint64 expected, uint64 observed);
    error Expired(uint64 deadline);
    error TimestampOverflow();
    error Unauthorized();

    event ShotCommitmentRecorded(bytes32 indexed commitment, uint64 committedAt, address indexed relayer);
    event ShotRegistered(
        bytes32 indexed shotId,
        address indexed controller,
        bytes32 indexed head,
        bytes32 commitment,
        uint64 checkpointSequence,
        uint64 actionNonce,
        address relayer
    );
    event CheckpointAppended(
        bytes32 indexed shotId,
        bytes32 indexed previousHead,
        bytes32 indexed newHead,
        uint64 checkpointSequence,
        uint64 actionNonce,
        address relayer
    );
    event ShotTransferred(
        bytes32 indexed shotId,
        address indexed previousController,
        address indexed newController,
        bytes32 currentHead,
        uint64 checkpointSequence,
        uint64 actionNonce,
        address relayer
    );

    constructor() EIP712Domain("TOHSENO ShotRegistry", "2") {}

    function getShot(bytes32 shotId) external view returns (Shot memory) {
        Shot memory shot = _shots[shotId];
        if (shot.controller == address(0)) revert ShotNotFound(shotId);
        return shot;
    }

    function controllerOf(bytes32 shotId) external view returns (address) {
        return _shots[shotId].controller;
    }

    function headOf(bytes32 shotId) external view returns (bytes32) {
        return _shots[shotId].head;
    }

    function checkpointSequenceOf(bytes32 shotId) external view returns (uint64) {
        return _shots[shotId].checkpointSequence;
    }

    function nonceOf(bytes32 shotId) external view returns (uint64) {
        return _shots[shotId].nonce;
    }

    function getCommitment(bytes32 commitment) external view returns (RegistrationCommitment memory) {
        return _commitments[commitment];
    }

    /// @notice Returns the exact permissionless commitment required by `registerShot`.
    /// @dev The registry and chain coordinates prevent a copied commitment from being
    ///      consumed in another registry generation or on another chain.
    function registrationCommitment(address controller, bytes32 shotId, bytes32 salt, uint64 deadline)
        public
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                REGISTRATION_COMMITMENT_TYPEHASH, controller, shotId, salt, address(this), block.chainid, deadline
            )
        );
    }

    function hashRegisterShot(RegisterShotAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    REGISTER_SHOT_TYPEHASH,
                    action.shotId,
                    action.controller,
                    action.head,
                    action.salt,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function hashAppendCheckpoint(AppendCheckpointAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    APPEND_CHECKPOINT_TYPEHASH,
                    action.shotId,
                    action.previousHead,
                    action.newHead,
                    action.checkpointSequence,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function hashTransferShot(TransferShotAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    TRANSFER_SHOT_TYPEHASH,
                    action.shotId,
                    action.currentController,
                    action.newController,
                    action.currentHead,
                    action.checkpointSequence,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    /// @notice Records a permissionless Shot registration commitment.
    /// @return recorded False when the same live commitment already exists. Its original
    ///         timestamp is deliberately preserved so an observer cannot reset maturity.
    function commitShot(bytes32 commitment) external returns (bool recorded) {
        if (commitment == bytes32(0)) revert InvalidCommitment();

        RegistrationCommitment storage existing = _commitments[commitment];
        if (existing.exists && block.timestamp <= uint256(existing.committedAt) + MAX_COMMIT_AGE) {
            return false;
        }

        uint64 committedAt = _timestamp64();
        _commitments[commitment] = RegistrationCommitment({committedAt: committedAt, exists: true});
        emit ShotCommitmentRecorded(commitment, committedAt, msg.sender);
        return true;
    }

    /// @notice Reveals a matured commitment and registers checkpoint one for a Shot.
    function registerShot(RegisterShotAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        if (action.shotId == bytes32(0)) revert InvalidShotId();
        if (action.head == bytes32(0)) revert InvalidHead();
        _checkController(action.controller);
        if (_shots[action.shotId].controller != address(0)) {
            revert ShotAlreadyExists(action.shotId);
        }

        uint64 expectedNonce = registrationNonces[action.controller];
        if (action.nonce != expectedNonce) revert InvalidNonce(expectedNonce, action.nonce);

        bytes32 commitment = registrationCommitment(action.controller, action.shotId, action.salt, action.deadline);
        RegistrationCommitment memory committed = _commitments[commitment];
        if (!committed.exists) revert CommitmentNotFound(commitment);

        uint256 availableAt = uint256(committed.committedAt) + MIN_COMMIT_AGE;
        if (block.timestamp < availableAt) revert CommitmentTooYoung(availableAt);
        uint256 expiredAt = uint256(committed.committedAt) + MAX_COMMIT_AGE;
        if (block.timestamp > expiredAt) revert CommitmentExpired(expiredAt);

        _requireValidSignature(action.controller, hashRegisterShot(action), signature);

        delete _commitments[commitment];
        registrationNonces[action.controller] = expectedNonce + 1;
        _shots[action.shotId] =
            Shot({controller: action.controller, head: action.head, checkpointSequence: 1, nonce: 1});
        emit ShotRegistered(action.shotId, action.controller, action.head, commitment, 1, action.nonce, msg.sender);
    }

    function appendCheckpoint(AppendCheckpointAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        Shot storage shot = _existingShot(action.shotId);
        if (action.previousHead != shot.head) {
            revert StaleHead(shot.head, action.previousHead);
        }
        if (action.newHead == bytes32(0) || action.newHead == action.previousHead) {
            revert InvalidHead();
        }
        uint64 expectedSequence = shot.checkpointSequence + 1;
        if (action.checkpointSequence != expectedSequence) {
            revert InvalidCheckpointSequence(expectedSequence, action.checkpointSequence);
        }
        if (action.nonce != shot.nonce) revert InvalidNonce(shot.nonce, action.nonce);
        _requireValidSignature(shot.controller, hashAppendCheckpoint(action), signature);

        shot.head = action.newHead;
        shot.checkpointSequence = action.checkpointSequence;
        shot.nonce += 1;
        emit CheckpointAppended(
            action.shotId, action.previousHead, action.newHead, action.checkpointSequence, action.nonce, msg.sender
        );
    }

    function transferShot(TransferShotAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        Shot storage shot = _existingShot(action.shotId);
        address currentController = shot.controller;
        if (action.currentController != currentController) {
            revert InvalidController(action.currentController);
        }
        _checkController(action.newController);
        if (action.newController == currentController) revert InvalidController(action.newController);
        if (action.currentHead != shot.head) revert StaleHead(shot.head, action.currentHead);
        if (action.checkpointSequence != shot.checkpointSequence) {
            revert InvalidCheckpointSequence(shot.checkpointSequence, action.checkpointSequence);
        }
        if (action.nonce != shot.nonce) revert InvalidNonce(shot.nonce, action.nonce);
        _requireValidSignature(currentController, hashTransferShot(action), signature);

        shot.controller = action.newController;
        shot.nonce += 1;
        emit ShotTransferred(
            action.shotId,
            currentController,
            action.newController,
            action.currentHead,
            action.checkpointSequence,
            action.nonce,
            msg.sender
        );
    }

    function _existingShot(bytes32 shotId) private view returns (Shot storage shot) {
        shot = _shots[shotId];
        if (shot.controller == address(0)) revert ShotNotFound(shotId);
    }

    function _checkController(address controller) private view {
        if (controller == address(0) || controller.code.length == 0) {
            revert InvalidController(controller);
        }

        bytes32 firstWord;
        assembly ("memory-safe") {
            extcodecopy(controller, 0, 0, 3)
            firstWord := mload(0)
        }
        if (controller.code.length == 23 && bytes3(firstWord) == EIP7702_DELEGATION_PREFIX) {
            revert DelegatedEOAController(controller);
        }
    }

    function _checkDeadline(uint64 deadline) private view {
        if (block.timestamp > deadline) revert Expired(deadline);
    }

    function _timestamp64() private view returns (uint64) {
        if (block.timestamp > type(uint64).max) revert TimestampOverflow();
        return uint64(block.timestamp);
    }

    function _requireValidSignature(address controller, bytes32 digest, bytes calldata signature) private view {
        (bool success, bytes memory output) =
            controller.staticcall(abi.encodeCall(IERC1271.isValidSignature, (digest, signature)));
        if (!success || output.length != 32) revert Unauthorized();

        bytes4 result;
        assembly ("memory-safe") {
            result := mload(add(output, 32))
        }
        if (result != ERC1271_MAGIC_VALUE) revert Unauthorized();
    }
}
