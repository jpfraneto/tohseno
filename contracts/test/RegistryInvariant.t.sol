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
    uint64 public modelCheckpointSequence;
    uint64 public modelNonce;
    uint64 public successfulAppends;
    uint64 public registrationAttempts;
    uint64 public successfulRegistrations;
    bool public invalidRegistrationSucceeded;
    bool public validRegistrationFailed;

    constructor(
        ShotRegistry registry_,
        bytes32 shotId_,
        address controller_,
        bytes32 initialHead,
        bytes memory signature_
    ) {
        registry = registry_;
        shotId = shotId_;
        controller = controller_;
        modelHead = initialHead;
        modelCheckpointSequence = 1;
        modelNonce = 1;
        signature = signature_;
    }

    function append(bytes32 proposedHead) external {
        bytes32 nextHead = proposedHead;
        if (nextHead == bytes32(0) || nextHead == modelHead) {
            nextHead = keccak256(abi.encodePacked(modelHead, modelCheckpointSequence));
        }
        ShotRegistry.AppendCheckpointAction memory action = ShotRegistry.AppendCheckpointAction({
            shotId: shotId,
            previousHead: modelHead,
            newHead: nextHead,
            checkpointSequence: modelCheckpointSequence + 1,
            nonce: modelNonce,
            deadline: type(uint64).max
        });
        _expectDigest(registry.hashAppendCheckpoint(action));
        registry.appendCheckpoint(action, signature);

        modelHead = nextHead;
        modelCheckpointSequence += 1;
        modelNonce += 1;
        successfulAppends += 1;
    }

    /// @dev Stateful registration model: 0=no commit, 1=too young,
    ///      2=mature and live, 3=expired.
    function attemptRegistration(bytes32 seed, uint8 rawMode, uint32 rawAge) external {
        uint8 mode = rawMode % 4;
        uint64 attempt = registrationAttempts;
        registrationAttempts = attempt + 1;
        bytes32 candidateShotId = keccak256(abi.encodePacked("registration-probe", seed, attempt));
        ShotRegistry.RegisterShotAction memory action = ShotRegistry.RegisterShotAction({
            shotId: candidateShotId,
            controller: controller,
            head: keccak256(abi.encodePacked("registration-head", candidateShotId)),
            salt: keccak256(abi.encodePacked("registration-salt", candidateShotId)),
            nonce: registry.registrationNonces(controller),
            deadline: type(uint64).max
        });

        bool shouldSucceed;
        if (mode != 0) {
            registry.commitShot(
                registry.registrationCommitment(action.controller, action.shotId, action.salt, action.deadline)
            );
            uint256 age;
            if (mode == 1) {
                age = uint256(rawAge) % registry.MIN_COMMIT_AGE();
            } else if (mode == 2) {
                age = registry.MIN_COMMIT_AGE()
                    + uint256(rawAge) % (registry.MAX_COMMIT_AGE() - registry.MIN_COMMIT_AGE() + 1);
                shouldSucceed = true;
            } else {
                age = registry.MAX_COMMIT_AGE() + 1 + uint256(rawAge) % 1 hours;
            }
            vm.warp(block.timestamp + age);
        }

        _expectDigest(registry.hashRegisterShot(action));
        (bool success,) =
            address(registry).call(abi.encodeWithSelector(ShotRegistry.registerShot.selector, action, signature));
        if (success) {
            successfulRegistrations += 1;
            if (!shouldSucceed) invalidRegistrationSucceeded = true;
        } else if (shouldSucceed) {
            validRegistrationFailed = true;
        }
    }

    function _expectDigest(bytes32 digest) private {
        vm.store(P256_PRECOMPILE, bytes32(uint256(0)), digest);
        vm.store(P256_PRECOMPILE, bytes32(uint256(1)), bytes32(0));
    }
}

contract RegistryInvariantTest is ProtocolTestBase {
    bytes32 private constant SHOT_ID = keccak256("invariant-shot");
    bytes32 private constant INITIAL_HEAD = keccak256("invariant-head");
    bytes32 private constant SALT = keccak256("invariant-salt");

    BuilderAccount private account;
    ShotRegistry private registry;
    RegistryHandler private handler;

    function setUp() public {
        installP256Mock();
        account = newAccount(KEY1_X, KEY1_Y);
        registry = new ShotRegistry();
        ShotRegistry.RegisterShotAction memory action = ShotRegistry.RegisterShotAction({
            shotId: SHOT_ID,
            controller: address(account),
            head: INITIAL_HEAD,
            salt: SALT,
            nonce: 0,
            deadline: type(uint64).max
        });
        registry.commitShot(
            registry.registrationCommitment(action.controller, action.shotId, action.salt, action.deadline)
        );
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());
        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));

        handler = new RegistryHandler(registry, SHOT_ID, address(account), INITIAL_HEAD, p256Signature(KEY1_X, KEY1_Y));
    }

    function testFuzzRegistryMatchesAppendOnlyCheckpointModel(bytes32 seed, uint8 rawSteps) public {
        uint256 steps = uint256(rawSteps) % 24 + 1;
        for (uint256 i = 0; i < steps; i++) {
            handler.append(keccak256(abi.encodePacked(seed, i)));
            _assertModel();
        }
    }

    function invariantCheckpointSequenceIsExactlyOnePlusSuccessfulAppends() public view {
        _assertModel();
    }

    function invariantRegistrationRequiresAMatureLiveCommitment() public view {
        assertFalse(handler.invalidRegistrationSucceeded());
        assertFalse(handler.validRegistrationFailed());
    }

    function targetContracts() public view returns (address[] memory targets) {
        targets = new address[](1);
        targets[0] = address(handler);
    }

    function testFuzzShotCannotRegisterWithoutItsMaturedCommitment(bytes32 shotId, bytes32 salt) public {
        vm.assume(shotId != bytes32(0));
        vm.assume(shotId != SHOT_ID);
        ShotRegistry.RegisterShotAction memory action = ShotRegistry.RegisterShotAction({
            shotId: shotId,
            controller: address(account),
            head: keccak256(abi.encodePacked(shotId, salt)),
            salt: salt,
            nonce: registry.registrationNonces(address(account)),
            deadline: type(uint64).max
        });
        vm.assume(action.head != bytes32(0));
        setP256Expected(registry.hashRegisterShot(action));
        vm.expectPartialRevert(ShotRegistry.CommitmentNotFound.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function _assertModel() private view {
        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(account));
        assertEq(shot.head, handler.modelHead());
        assertEq(shot.checkpointSequence, handler.modelCheckpointSequence());
        assertEq(shot.nonce, handler.modelNonce());
        assertEq(shot.checkpointSequence, uint64(handler.successfulAppends() + 1));
    }
}
