// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {EIP712Domain} from "./EIP712Domain.sol";
import {IERC1271} from "./IERC1271.sol";

/// @notice Neutral public witness for TOHSENO Shot ownership and lineage heads.
/// @dev Any address may relay a valid signed action. `msg.sender` is never authority.
contract ShotRegistry is EIP712Domain {
    bytes4 private constant ERC1271_MAGIC_VALUE = 0x1626ba7e;
    uint8 public constant PUBLIC_STATE_PUBLISHED = 1;
    uint8 public constant PUBLIC_STATE_APP_STORE = 2;

    bytes32 public constant CREATE_SHOT_TYPEHASH = keccak256(
        "CreateShot(bytes32 shotId,address controller,bytes32 head,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant APPEND_EVOLUTION_TYPEHASH = keccak256(
        "AppendEvolution(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 sequence,bytes32 contentCommitment,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant TRANSFER_SHOT_TYPEHASH = keccak256(
        "TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 sequence,uint64 nonce,uint64 deadline)"
    );
    bytes32 public constant SET_PUBLIC_STATE_TYPEHASH = keccak256(
        "SetPublicState(bytes32 shotId,bytes32 currentHead,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)"
    );

    struct Shot {
        address controller;
        bytes32 head;
        bytes32 contentCommitment;
        uint64 sequence;
        uint64 nonce;
        uint8 publicState;
    }

    struct CreateShotAction {
        bytes32 shotId;
        address controller;
        bytes32 head;
        uint64 sequence;
        uint8 publicState;
        bytes32 contentCommitment;
        uint64 nonce;
        uint64 deadline;
    }

    struct AppendEvolutionAction {
        bytes32 shotId;
        bytes32 previousHead;
        bytes32 newHead;
        uint64 sequence;
        bytes32 contentCommitment;
        uint64 nonce;
        uint64 deadline;
    }

    struct TransferShotAction {
        bytes32 shotId;
        address currentController;
        address newController;
        bytes32 currentHead;
        uint64 sequence;
        uint64 nonce;
        uint64 deadline;
    }

    struct SetPublicStateAction {
        bytes32 shotId;
        bytes32 currentHead;
        uint64 sequence;
        uint8 publicState;
        bytes32 contentCommitment;
        uint64 nonce;
        uint64 deadline;
    }

    mapping(bytes32 shotId => Shot) private _shots;
    mapping(address controller => uint64 nonce) public createNonces;

    error InvalidShotId();
    error InvalidHead();
    error InvalidController(address controller);
    error InvalidPublicState(uint8 publicState);
    error InvalidPublicStateTransition(uint8 currentState, uint8 requestedState);
    error ShotAlreadyExists(bytes32 shotId);
    error ShotNotFound(bytes32 shotId);
    error InvalidSequence(uint64 expected, uint64 observed);
    error StaleHead(bytes32 expected, bytes32 observed);
    error InvalidNonce(uint64 expected, uint64 observed);
    error Expired(uint64 deadline);
    error Unauthorized();

    event ShotCreated(
        bytes32 indexed shotId,
        address indexed controller,
        bytes32 indexed head,
        uint64 sequence,
        uint8 publicState,
        bytes32 contentCommitment,
        uint64 actionNonce,
        address relayer
    );
    event EvolutionAppended(
        bytes32 indexed shotId,
        bytes32 indexed previousHead,
        bytes32 indexed newHead,
        uint64 sequence,
        bytes32 contentCommitment,
        uint64 actionNonce,
        address relayer
    );
    event ShotTransferred(
        bytes32 indexed shotId,
        address indexed previousController,
        address indexed newController,
        bytes32 currentHead,
        uint64 sequence,
        uint64 actionNonce,
        address relayer
    );
    event PublicStateSet(
        bytes32 indexed shotId,
        uint8 previousState,
        uint8 newState,
        bytes32 indexed head,
        bytes32 contentCommitment,
        uint64 actionNonce,
        address relayer
    );

    constructor() EIP712Domain("TOHSENO ShotRegistry", "1") {}

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

    function sequenceOf(bytes32 shotId) external view returns (uint64) {
        return _shots[shotId].sequence;
    }

    function nonceOf(bytes32 shotId) external view returns (uint64) {
        return _shots[shotId].nonce;
    }

    function hashCreateShot(CreateShotAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    CREATE_SHOT_TYPEHASH,
                    action.shotId,
                    action.controller,
                    action.head,
                    action.sequence,
                    action.publicState,
                    action.contentCommitment,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function hashAppendEvolution(AppendEvolutionAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    APPEND_EVOLUTION_TYPEHASH,
                    action.shotId,
                    action.previousHead,
                    action.newHead,
                    action.sequence,
                    action.contentCommitment,
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
                    action.sequence,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function hashSetPublicState(SetPublicStateAction calldata action) public view returns (bytes32) {
        return _hashTypedData(
            keccak256(
                abi.encode(
                    SET_PUBLIC_STATE_TYPEHASH,
                    action.shotId,
                    action.currentHead,
                    action.sequence,
                    action.publicState,
                    action.contentCommitment,
                    action.nonce,
                    action.deadline
                )
            )
        );
    }

    function createShot(CreateShotAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        if (action.shotId == bytes32(0)) revert InvalidShotId();
        if (action.head == bytes32(0)) revert InvalidHead();
        _checkController(action.controller);
        if (action.publicState != PUBLIC_STATE_PUBLISHED) {
            revert InvalidPublicState(action.publicState);
        }
        if (_shots[action.shotId].controller != address(0)) {
            revert ShotAlreadyExists(action.shotId);
        }
        // Native roots start at 1; adopted legacy roots preserve their exact N + 1 sequence.
        if (action.sequence == 0) revert InvalidSequence(1, action.sequence);

        uint64 expectedNonce = createNonces[action.controller];
        if (action.nonce != expectedNonce) revert InvalidNonce(expectedNonce, action.nonce);
        _requireValidSignature(action.controller, hashCreateShot(action), signature);

        createNonces[action.controller] = expectedNonce + 1;
        _shots[action.shotId] = Shot({
            controller: action.controller,
            head: action.head,
            contentCommitment: action.contentCommitment,
            sequence: action.sequence,
            nonce: 1,
            publicState: action.publicState
        });
        emit ShotCreated(
            action.shotId,
            action.controller,
            action.head,
            action.sequence,
            action.publicState,
            action.contentCommitment,
            action.nonce,
            msg.sender
        );
    }

    function appendEvolution(AppendEvolutionAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        Shot storage shot = _existingShot(action.shotId);
        if (action.previousHead != shot.head) {
            revert StaleHead(shot.head, action.previousHead);
        }
        if (action.newHead == bytes32(0) || action.newHead == action.previousHead) {
            revert InvalidHead();
        }
        uint64 expectedSequence = shot.sequence + 1;
        if (action.sequence != expectedSequence) {
            revert InvalidSequence(expectedSequence, action.sequence);
        }
        if (action.nonce != shot.nonce) revert InvalidNonce(shot.nonce, action.nonce);
        _requireValidSignature(shot.controller, hashAppendEvolution(action), signature);

        shot.head = action.newHead;
        shot.contentCommitment = action.contentCommitment;
        shot.sequence = action.sequence;
        shot.nonce += 1;
        emit EvolutionAppended(
            action.shotId,
            action.previousHead,
            action.newHead,
            action.sequence,
            action.contentCommitment,
            action.nonce,
            msg.sender
        );
    }

    function transferShot(TransferShotAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        Shot storage shot = _existingShot(action.shotId);
        if (action.currentController != shot.controller) {
            revert InvalidController(action.currentController);
        }
        _checkController(action.newController);
        if (action.newController == shot.controller) revert InvalidController(action.newController);
        if (action.currentHead != shot.head) revert StaleHead(shot.head, action.currentHead);
        if (action.sequence != shot.sequence) {
            revert InvalidSequence(shot.sequence, action.sequence);
        }
        if (action.nonce != shot.nonce) revert InvalidNonce(shot.nonce, action.nonce);
        _requireValidSignature(shot.controller, hashTransferShot(action), signature);

        address previousController = shot.controller;
        shot.controller = action.newController;
        shot.nonce += 1;
        emit ShotTransferred(
            action.shotId,
            previousController,
            action.newController,
            action.currentHead,
            action.sequence,
            action.nonce,
            msg.sender
        );
    }

    function setPublicState(SetPublicStateAction calldata action, bytes calldata signature) external {
        _checkDeadline(action.deadline);
        _checkPublicState(action.publicState);
        Shot storage shot = _existingShot(action.shotId);
        if (action.publicState < shot.publicState) {
            revert InvalidPublicStateTransition(shot.publicState, action.publicState);
        }
        if (action.currentHead != shot.head) revert StaleHead(shot.head, action.currentHead);
        if (action.sequence != shot.sequence) {
            revert InvalidSequence(shot.sequence, action.sequence);
        }
        if (action.nonce != shot.nonce) revert InvalidNonce(shot.nonce, action.nonce);
        _requireValidSignature(shot.controller, hashSetPublicState(action), signature);

        uint8 previousState = shot.publicState;
        shot.publicState = action.publicState;
        shot.contentCommitment = action.contentCommitment;
        shot.nonce += 1;
        emit PublicStateSet(
            action.shotId,
            previousState,
            action.publicState,
            action.currentHead,
            action.contentCommitment,
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
    }

    function _checkPublicState(uint8 publicState) private pure {
        if (publicState != PUBLIC_STATE_PUBLISHED && publicState != PUBLIC_STATE_APP_STORE) {
            revert InvalidPublicState(publicState);
        }
    }

    function _checkDeadline(uint64 deadline) private view {
        if (block.timestamp > deadline) revert Expired(deadline);
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
