// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {Vm} from "./TestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";

contract RegistryHandler {
    Vm private constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant P256_PRECOMPILE = address(0x100);

    ShotRegistry public immutable registry;
    bytes32 public immutable shotId;
    address public immutable controller;
    bytes public signature;

    bytes32 public modelHead;
    bytes32 public modelContent;
    uint64 public modelSequence;
    uint64 public modelNonce;
    uint8 public modelState;
    uint64 public successfulAppends;

    constructor(
        ShotRegistry registry_,
        bytes32 shotId_,
        address controller_,
        bytes32 initialHead,
        bytes32 initialContent,
        uint64 initialSequence,
        bytes memory signature_
    ) {
        registry = registry_;
        shotId = shotId_;
        controller = controller_;
        modelHead = initialHead;
        modelContent = initialContent;
        modelSequence = initialSequence;
        modelNonce = 1;
        modelState = registry_.PUBLIC_STATE_PUBLISHED();
        signature = signature_;
    }

    function append(bytes32 proposedHead, bytes32 contentCommitment) external {
        bytes32 nextHead = proposedHead;
        if (nextHead == bytes32(0) || nextHead == modelHead) {
            nextHead = keccak256(abi.encodePacked(modelHead, contentCommitment, modelSequence));
        }
        ShotRegistry.AppendEvolutionAction memory action = ShotRegistry.AppendEvolutionAction({
            shotId: shotId,
            previousHead: modelHead,
            newHead: nextHead,
            sequence: modelSequence + 1,
            contentCommitment: contentCommitment,
            nonce: modelNonce,
            deadline: type(uint64).max
        });
        _expectDigest(registry.hashAppendEvolution(action));
        registry.appendEvolution(action, signature);

        modelHead = nextHead;
        modelContent = contentCommitment;
        modelSequence += 1;
        modelNonce += 1;
        successfulAppends += 1;
    }

    function setState(bool graduate, bytes32 contentCommitment) external {
        uint8 nextState = modelState;
        if (graduate) nextState = registry.PUBLIC_STATE_APP_STORE();
        ShotRegistry.SetPublicStateAction memory action = ShotRegistry.SetPublicStateAction({
            shotId: shotId,
            currentHead: modelHead,
            sequence: modelSequence,
            publicState: nextState,
            contentCommitment: contentCommitment,
            nonce: modelNonce,
            deadline: type(uint64).max
        });
        _expectDigest(registry.hashSetPublicState(action));
        registry.setPublicState(action, signature);

        modelContent = contentCommitment;
        modelState = nextState;
        modelNonce += 1;
    }

    function _expectDigest(bytes32 digest) private {
        vm.store(P256_PRECOMPILE, bytes32(uint256(0)), digest);
        vm.store(P256_PRECOMPILE, bytes32(uint256(1)), bytes32(0));
    }
}

contract RegistryInvariantTest is ProtocolTestBase {
    uint64 private constant INITIAL_SEQUENCE = 8;
    bytes32 private constant SHOT_ID = keccak256("invariant-shot");
    bytes32 private constant INITIAL_HEAD = keccak256("invariant-head");
    bytes32 private constant INITIAL_CONTENT = keccak256("invariant-content");

    BuilderAccount private account;
    ShotRegistry private registry;
    RegistryHandler private handler;

    function setUp() public {
        installP256Mock();
        account = newAccount(KEY1_X, KEY1_Y);
        registry = new ShotRegistry();
        ShotRegistry.CreateShotAction memory action = ShotRegistry.CreateShotAction({
            shotId: SHOT_ID,
            controller: address(account),
            head: INITIAL_HEAD,
            sequence: INITIAL_SEQUENCE,
            publicState: registry.PUBLIC_STATE_PUBLISHED(),
            contentCommitment: INITIAL_CONTENT,
            nonce: 0,
            deadline: type(uint64).max
        });
        setP256Expected(registry.hashCreateShot(action));
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));

        handler = new RegistryHandler(
            registry,
            SHOT_ID,
            address(account),
            INITIAL_HEAD,
            INITIAL_CONTENT,
            INITIAL_SEQUENCE,
            p256Signature(KEY1_X, KEY1_Y)
        );
    }

    function testFuzzRegistryMatchesTheTransitionModel(bytes32 seed, uint8 rawSteps) public {
        uint256 steps = uint256(rawSteps) % 24 + 1;
        for (uint256 i = 0; i < steps; i++) {
            bytes32 decision = keccak256(abi.encodePacked(seed, i));
            bytes32 content = keccak256(abi.encodePacked("content", decision));
            if (uint256(decision) & 1 == 0) {
                handler.append(keccak256(abi.encodePacked("head", decision)), content);
            } else {
                handler.setState(uint256(decision) & 2 != 0, content);
            }
            _assertModel();
        }
    }

    function testInitialPublicStateIsPublishedAndNeverLocal() public view {
        _assertModel();
        assertEq(registry.getShot(SHOT_ID).publicState, registry.PUBLIC_STATE_PUBLISHED());
    }

    function _assertModel() private view {
        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(account));
        assertEq(shot.head, handler.modelHead());
        assertEq(shot.contentCommitment, handler.modelContent());
        assertEq(shot.sequence, handler.modelSequence());
        assertEq(shot.nonce, handler.modelNonce());
        assertEq(shot.publicState, handler.modelState());
        assertEq(shot.sequence, uint64(handler.successfulAppends() + INITIAL_SEQUENCE));
        assertTrue(
            shot.publicState == registry.PUBLIC_STATE_PUBLISHED()
                || shot.publicState == registry.PUBLIC_STATE_APP_STORE()
        );
    }
}
