// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {BuilderAccountFactory} from "../src/BuilderAccountFactory.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";

contract ConfigurableERC1271Controller {
    bytes4 private constant MAGIC = 0x1626ba7e;
    bytes4 private constant INVALID = 0xffffffff;

    bytes32 public expectedDigest;
    uint8 public mode;

    function configure(bytes32 expectedDigest_, uint8 mode_) external {
        expectedDigest = expectedDigest_;
        mode = mode_;
    }

    function isValidSignature(bytes32 digest, bytes calldata) external view returns (bytes4) {
        uint8 currentMode = mode;
        if (currentMode == 2) {
            assembly ("memory-safe") {
                return(0, 0)
            }
        }
        if (currentMode == 3) {
            assembly ("memory-safe") {
                mstore(0, 0)
                return(0, 31)
            }
        }
        if (currentMode == 4) {
            assembly ("memory-safe") {
                mstore(0, 0)
                mstore(32, 0)
                return(0, 64)
            }
        }
        if (currentMode == 5) {
            assembly ("memory-safe") {
                revert(0, 0)
            }
        }
        if (currentMode == 1 || digest != expectedDigest) return INVALID;
        return MAGIC;
    }
}

contract ShotRegistryTest is ProtocolTestBase {
    bytes32 private constant SHOT_ID = keccak256("shot-1");
    bytes32 private constant HEAD_1 = keccak256("head-1");
    bytes32 private constant HEAD_2 = keccak256("head-2");
    bytes32 private constant SALT = keccak256("registration-salt");
    address private constant RELAYER = address(0xbee);

    BuilderAccount private account;
    BuilderAccount private nextAccount;
    ShotRegistry private registry;

    function setUp() public {
        installP256Mock();
        account = newAccount(KEY1_X, KEY1_Y);
        nextAccount = newAccount(KEY2_X, KEY2_Y);
        registry = new ShotRegistry();
    }

    function testPermissionlessCommitAndRelayedRevealNeverAuthorizeCaller() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        bytes32 commitment = _commitment(action);

        vm.prank(address(0x1111));
        assertTrue(registry.commitShot(commitment));
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());

        setP256Expected(registry.hashRegisterShot(action));
        vm.prank(RELAYER);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(account));
        assertTrue(shot.controller != RELAYER);
        assertEq(shot.head, HEAD_1);
        assertEq(shot.checkpointSequence, uint64(1));
        assertEq(shot.nonce, uint64(1));
        assertEq(registry.registrationNonces(address(account)), uint64(1));
    }

    function testShotIdRemainsRandomPortableIdentityNotControllerDerived() public {
        bytes32 externallyGenerated = keccak256(abi.encodePacked("independent", uint256(91)));
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        action.shotId = externallyGenerated;
        _commitAndMature(action);
        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));

        assertEq(registry.controllerOf(externallyGenerated), address(account));
        assertTrue(externallyGenerated != keccak256(abi.encode(address(account), SALT)));
    }

    function testCannotRegisterWithoutCommitment() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        vm.expectPartialRevert(ShotRegistry.CommitmentNotFound.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsZeroCommitmentShotIdAndHead() public {
        vm.expectRevert(ShotRegistry.InvalidCommitment.selector);
        registry.commitShot(bytes32(0));

        ShotRegistry.RegisterShotAction memory zeroShot = _registerAction(address(account));
        zeroShot.shotId = bytes32(0);
        vm.expectRevert(ShotRegistry.InvalidShotId.selector);
        registry.registerShot(zeroShot, hex"");

        ShotRegistry.RegisterShotAction memory zeroHead = _registerAction(address(account));
        zeroHead.head = bytes32(0);
        vm.expectRevert(ShotRegistry.InvalidHead.selector);
        registry.registerShot(zeroHead, hex"");
    }

    function testCannotRegisterBeforeCommitmentMatures() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        registry.commitShot(_commitment(action));

        vm.expectPartialRevert(ShotRegistry.CommitmentTooYoung.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRevealAllowedAtExactMinimumAgeAndDeadline() public {
        uint64 committedAt = _now64();
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        action.deadline = committedAt + registry.MIN_COMMIT_AGE();
        registry.commitShot(_commitment(action));
        vm.warp(action.deadline);
        setP256Expected(registry.hashRegisterShot(action));

        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testRevealAllowedAtExactMaximumAge() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        action.deadline = type(uint64).max;
        uint64 committedAt = _now64();
        registry.commitShot(_commitment(action));
        vm.warp(uint256(committedAt) + registry.MAX_COMMIT_AGE());
        assertFalse(registry.commitShot(_commitment(action)));
        setP256Expected(registry.hashRegisterShot(action));

        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testRevealRejectsAfterMaximumAge() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        action.deadline = type(uint64).max;
        uint64 committedAt = _now64();
        registry.commitShot(_commitment(action));
        vm.warp(uint256(committedAt) + registry.MAX_COMMIT_AGE() + 1);

        vm.expectPartialRevert(ShotRegistry.CommitmentExpired.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRevealRejectsAfterSignedDeadlineEvenInsideCommitWindow() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        uint64 committedAt = _now64();
        action.deadline = committedAt + registry.MIN_COMMIT_AGE();
        registry.commitShot(_commitment(action));
        vm.warp(uint256(action.deadline) + 1);

        vm.expectPartialRevert(ShotRegistry.Expired.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testDuplicateLiveCommitIsIdempotentAndCannotResetMaturity() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        bytes32 commitment = _commitment(action);
        uint64 committedAt = _now64();
        assertTrue(registry.commitShot(commitment));

        vm.warp(uint256(committedAt) + registry.MIN_COMMIT_AGE());
        vm.prank(address(0xa11ce));
        assertFalse(registry.commitShot(commitment));
        ShotRegistry.RegistrationCommitment memory observed = registry.getCommitment(commitment);
        assertEq(observed.committedAt, committedAt);
        assertTrue(observed.exists);

        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testExpiredCommitCanBeRecordedAgainWithFreshMaturity() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        action.deadline = type(uint64).max;
        bytes32 commitment = _commitment(action);
        uint64 firstCommittedAt = _now64();
        registry.commitShot(commitment);
        vm.warp(uint256(firstCommittedAt) + registry.MAX_COMMIT_AGE() + 1);

        assertTrue(registry.commitShot(commitment));
        ShotRegistry.RegistrationCommitment memory observed = registry.getCommitment(commitment);
        assertEq(observed.committedAt, _now64());
        vm.expectPartialRevert(ShotRegistry.CommitmentTooYoung.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testSuccessfulRevealDeletesConsumedCommitment() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        bytes32 commitment = _commitAndMature(action);
        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.RegistrationCommitment memory observed = registry.getCommitment(commitment);
        assertEq(observed.committedAt, uint64(0));
        assertFalse(observed.exists);
    }

    function testObservedRevealCannotBeFrontRunByDifferentController() public {
        ShotRegistry.RegisterShotAction memory victim = _registerAction(address(account));
        _commitAndMature(victim);

        ShotRegistry.RegisterShotAction memory attacker = _registerAction(address(nextAccount));
        bytes32 attackerCommitment = _commitment(attacker);
        vm.prank(address(0xbad));
        registry.commitShot(attackerCommitment);

        vm.expectPartialRevert(ShotRegistry.CommitmentTooYoung.selector);
        registry.registerShot(attacker, p256Signature(KEY2_X, KEY2_Y));

        setP256Expected(registry.hashRegisterShot(victim));
        registry.registerShot(victim, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testCopiedVictimCommitmentCannotRevealAsVictimWithoutSignature() public {
        ShotRegistry.RegisterShotAction memory victim = _registerAction(address(account));
        bytes32 commitment = _commitment(victim);
        vm.prank(address(0xbad));
        registry.commitShot(commitment);
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());

        setP256Expected(registry.hashRegisterShot(victim));
        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.registerShot(victim, p256Signature(KEY2_X, KEY2_Y));

        registry.registerShot(victim, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testCounterfactualControllerCanCommitBeforeDeploymentThenReveal() public {
        BuilderAccountFactory factory = new BuilderAccountFactory();
        bytes32 accountSalt = keccak256("counterfactual-account");
        address counterfactual = factory.predictAccount(accountSalt, KEY3_X, KEY3_Y);
        ShotRegistry.RegisterShotAction memory action = _registerAction(counterfactual);
        assertTrue(registry.commitShot(_commitment(action)));
        assertEq(counterfactual.code.length, uint256(0));

        BuilderAccount deployed = factory.createAccount(accountSalt, KEY3_X, KEY3_Y);
        assertEq(address(deployed), counterfactual);
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());
        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(KEY3_X, KEY3_Y));
        assertEq(registry.controllerOf(SHOT_ID), counterfactual);
    }

    function testRevealRejectsUndeployedAndEip7702Controllers() public {
        ShotRegistry.RegisterShotAction memory undeployed = _registerAction(address(0xc0ffee));
        _commitAndMature(undeployed);
        vm.expectPartialRevert(ShotRegistry.InvalidController.selector);
        registry.registerShot(undeployed, hex"");

        address delegated = address(0x7702);
        vm.etch(delegated, abi.encodePacked(hex"ef0100", bytes20(address(account))));
        ShotRegistry.RegisterShotAction memory delegatedAction = _registerAction(delegated);
        delegatedAction.shotId = keccak256("delegated-shot");
        _commitAndMature(delegatedAction);
        vm.expectPartialRevert(ShotRegistry.DelegatedEOAController.selector);
        registry.registerShot(delegatedAction, hex"");
    }

    function testAppendCheckpointAdvancesHeadSequenceAndNonceExactlyOnce() public {
        _registerDefaultShot();
        ShotRegistry.AppendCheckpointAction memory action = _appendAction(HEAD_1, HEAD_2, 2, 1);
        setP256Expected(registry.hashAppendCheckpoint(action));
        vm.prank(RELAYER);
        registry.appendCheckpoint(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(account));
        assertEq(shot.head, HEAD_2);
        assertEq(shot.checkpointSequence, uint64(2));
        assertEq(shot.nonce, uint64(2));
    }

    function testNeutralRegistryAcceptsAnArbitraryWellFormedERC1271Controller() public {
        ConfigurableERC1271Controller neutralController = new ConfigurableERC1271Controller();
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(neutralController));
        _commitAndMature(action);
        neutralController.configure(registry.hashRegisterShot(action), 0);

        registry.registerShot(action, bytes("any-controller-defined-signature"));
        assertEq(registry.controllerOf(SHOT_ID), address(neutralController));
    }

    function testMalformedERC1271ReturnsAndRevertsAreRejected() public {
        ConfigurableERC1271Controller neutralController = new ConfigurableERC1271Controller();
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(neutralController));
        _commitAndMature(action);
        bytes32 digest = registry.hashRegisterShot(action);

        for (uint8 mode = 1; mode <= 5; mode++) {
            neutralController.configure(digest, mode);
            vm.expectRevert(ShotRegistry.Unauthorized.selector);
            registry.registerShot(action, bytes("controller-defined"));
        }

        neutralController.configure(digest, 0);
        registry.registerShot(action, bytes("controller-defined"));
        assertEq(registry.controllerOf(SHOT_ID), address(neutralController));
    }

    function testCheckpointRejectsStaleHeadSkippedSequenceAndReplay() public {
        _registerDefaultShot();
        ShotRegistry.AppendCheckpointAction memory stale = _appendAction(keccak256("stale"), HEAD_2, 2, 1);
        vm.expectPartialRevert(ShotRegistry.StaleHead.selector);
        registry.appendCheckpoint(stale, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.AppendCheckpointAction memory skipped = _appendAction(HEAD_1, HEAD_2, 3, 1);
        vm.expectPartialRevert(ShotRegistry.InvalidCheckpointSequence.selector);
        registry.appendCheckpoint(skipped, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.AppendCheckpointAction memory valid = _appendAction(HEAD_1, HEAD_2, 2, 1);
        setP256Expected(registry.hashAppendCheckpoint(valid));
        registry.appendCheckpoint(valid, p256Signature(KEY1_X, KEY1_Y));
        vm.expectPartialRevert(ShotRegistry.StaleHead.selector);
        registry.appendCheckpoint(valid, p256Signature(KEY1_X, KEY1_Y));
    }

    function testSignedCheckpointAndTransferDeadlinesAreInclusiveThenExpire() public {
        _registerDefaultShot();
        ShotRegistry.AppendCheckpointAction memory append = _appendAction(HEAD_1, HEAD_2, 2, 1);
        append.deadline = _now64();
        setP256Expected(registry.hashAppendCheckpoint(append));
        registry.appendCheckpoint(append, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.TransferShotAction memory transfer = ShotRegistry.TransferShotAction({
            shotId: SHOT_ID,
            currentController: address(account),
            newController: address(nextAccount),
            currentHead: HEAD_2,
            checkpointSequence: 2,
            nonce: 2,
            deadline: _now64()
        });
        vm.warp(block.timestamp + 1);
        vm.expectPartialRevert(ShotRegistry.Expired.selector);
        registry.transferShot(transfer, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRegisteredShotIdCannotBeClaimedAgain() public {
        _registerDefaultShot();
        ShotRegistry.RegisterShotAction memory duplicate = _registerAction(address(nextAccount));
        duplicate.nonce = registry.registrationNonces(address(nextAccount));
        _commitAndMature(duplicate);

        vm.expectPartialRevert(ShotRegistry.ShotAlreadyExists.selector);
        registry.registerShot(duplicate, p256Signature(KEY2_X, KEY2_Y));
    }

    function testRegistrationNonceCannotReplayForAnotherShot() public {
        _registerDefaultShot();
        ShotRegistry.RegisterShotAction memory replay = _registerAction(address(account));
        replay.shotId = keccak256("second-shot");
        replay.head = keccak256("second-head");
        _commitAndMature(replay);

        vm.expectPartialRevert(ShotRegistry.InvalidNonce.selector);
        registry.registerShot(replay, p256Signature(KEY1_X, KEY1_Y));

        replay.nonce = 1;
        setP256Expected(registry.hashRegisterShot(replay));
        registry.registerShot(replay, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.registrationNonces(address(account)), uint64(2));
    }

    function testTransferChangesOnlyControllerAndInvalidatesOldPresignedAction() public {
        _registerDefaultShot();
        ShotRegistry.AppendCheckpointAction memory presigned = _appendAction(HEAD_1, HEAD_2, 2, 1);

        ShotRegistry.TransferShotAction memory transfer = ShotRegistry.TransferShotAction({
            shotId: SHOT_ID,
            currentController: address(account),
            newController: address(nextAccount),
            currentHead: HEAD_1,
            checkpointSequence: 1,
            nonce: 1,
            deadline: _deadline()
        });
        setP256Expected(registry.hashTransferShot(transfer));
        vm.prank(RELAYER);
        registry.transferShot(transfer, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(nextAccount));
        assertEq(shot.head, HEAD_1);
        assertEq(shot.checkpointSequence, uint64(1));
        assertEq(shot.nonce, uint64(2));

        vm.expectPartialRevert(ShotRegistry.InvalidNonce.selector);
        registry.appendCheckpoint(presigned, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.AppendCheckpointAction memory next = _appendAction(HEAD_1, HEAD_2, 2, 2);
        setP256Expected(registry.hashAppendCheckpoint(next));
        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.appendCheckpoint(next, p256Signature(KEY1_X, KEY1_Y));

        registry.appendCheckpoint(next, p256Signature(KEY2_X, KEY2_Y));
        assertEq(registry.headOf(SHOT_ID), HEAD_2);
    }

    function testTransferRejectsAnEip7702DelegatedEOAController() public {
        _registerDefaultShot();
        address delegated = address(0x7702);
        vm.etch(delegated, abi.encodePacked(hex"ef0100", bytes20(address(nextAccount))));
        ShotRegistry.TransferShotAction memory transfer = ShotRegistry.TransferShotAction({
            shotId: SHOT_ID,
            currentController: address(account),
            newController: delegated,
            currentHead: HEAD_1,
            checkpointSequence: 1,
            nonce: 1,
            deadline: _deadline()
        });

        vm.expectPartialRevert(ShotRegistry.DelegatedEOAController.selector);
        registry.transferShot(transfer, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRelayingVictimRevealFirstCanOnlyRegisterTheVictim() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        _commitAndMature(action);
        setP256Expected(registry.hashRegisterShot(action));

        vm.prank(address(0xbad));
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testRegisterSignatureCannotReplayAcrossRegistryDomains() public {
        ShotRegistry other = new ShotRegistry();
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        registry.commitShot(_commitment(action));
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());
        setP256Expected(other.hashRegisterShot(action));

        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRegisterSignatureCannotReplayAfterRealChainIdChange() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        bytes32 oldDigest = registry.hashRegisterShot(action);
        vm.chainId(block.chainid + 1);
        registry.commitShot(_commitment(action));
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());
        setP256Expected(oldDigest);

        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRegistrationCommitmentUsesExactDocumentedEncoding() public view {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        bytes32 expected = keccak256(
            abi.encode(
                registry.REGISTRATION_COMMITMENT_TYPEHASH(),
                action.controller,
                action.shotId,
                action.salt,
                address(registry),
                block.chainid,
                action.deadline
            )
        );
        assertEq(
            registry.registrationCommitment(action.controller, action.shotId, action.salt, action.deadline), expected
        );
    }

    function testEveryRegistrationCoordinateChangesTheCommitment() public {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        bytes32 baseline = _commitment(action);
        assertTrue(
            baseline
                != registry.registrationCommitment(address(nextAccount), action.shotId, action.salt, action.deadline)
        );
        assertTrue(
            baseline
                != registry.registrationCommitment(action.controller, keccak256("other-shot"), action.salt, action.deadline)
        );
        assertTrue(
            baseline
                != registry.registrationCommitment(
                    action.controller, action.shotId, keccak256("other-salt"), action.deadline
                )
        );
        assertTrue(
            baseline
                != registry.registrationCommitment(action.controller, action.shotId, action.salt, action.deadline + 1)
        );

        ShotRegistry otherRegistry = new ShotRegistry();
        assertTrue(
            baseline
                != otherRegistry.registrationCommitment(action.controller, action.shotId, action.salt, action.deadline)
        );

        uint256 originalChainId = block.chainid;
        vm.chainId(originalChainId + 1);
        assertTrue(baseline != _commitment(action));
    }

    function _registerDefaultShot() private {
        ShotRegistry.RegisterShotAction memory action = _registerAction(address(account));
        _commitAndMature(action);
        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function _commitAndMature(ShotRegistry.RegisterShotAction memory action) private returns (bytes32 commitment) {
        commitment = _commitment(action);
        registry.commitShot(commitment);
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());
    }

    function _commitment(ShotRegistry.RegisterShotAction memory action) private view returns (bytes32) {
        return registry.registrationCommitment(action.controller, action.shotId, action.salt, action.deadline);
    }

    function _registerAction(address controller) private view returns (ShotRegistry.RegisterShotAction memory) {
        return ShotRegistry.RegisterShotAction({
            shotId: SHOT_ID,
            controller: controller,
            head: HEAD_1,
            salt: SALT,
            nonce: 0,
            deadline: _deadline()
        });
    }

    function _appendAction(bytes32 previousHead, bytes32 newHead, uint64 sequence, uint64 nonce)
        private
        view
        returns (ShotRegistry.AppendCheckpointAction memory)
    {
        return ShotRegistry.AppendCheckpointAction({
            shotId: SHOT_ID,
            previousHead: previousHead,
            newHead: newHead,
            checkpointSequence: sequence,
            nonce: nonce,
            deadline: _deadline()
        });
    }

    function _deadline() private view returns (uint64) {
        return uint64(block.timestamp + 2 days);
    }

    function _now64() private view returns (uint64) {
        return uint64(block.timestamp);
    }
}
