// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";

contract ShotRegistryTest is ProtocolTestBase {
    bytes32 private constant SHOT_ID = keccak256("shot-1");
    bytes32 private constant HEAD_1 = keccak256("head-1");
    bytes32 private constant HEAD_2 = keccak256("head-2");
    bytes32 private constant CONTENT_1 = keccak256("content-1");
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

    function testAnyRelayerCanCreateWithoutBecomingOwner() public {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        bytes32 digest = registry.hashCreateShot(action);
        setP256Expected(digest);
        vm.prank(RELAYER);
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(account));
        assertTrue(shot.controller != RELAYER);
        assertEq(shot.head, HEAD_1);
        assertEq(shot.sequence, uint64(1));
        assertEq(shot.nonce, uint64(1));
        assertEq(shot.publicState, registry.PUBLIC_STATE_PUBLISHED());
    }

    function testAdoptedRootStartsAtLegacyNextSequence() public {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        action.sequence = 8;
        setP256Expected(registry.hashCreateShot(action));
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.sequence, uint64(8));

        ShotRegistry.AppendEvolutionAction memory appendAction = _appendAction(HEAD_1, HEAD_2, 9, 1);
        setP256Expected(registry.hashAppendEvolution(appendAction));
        registry.appendEvolution(appendAction, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.sequenceOf(SHOT_ID), uint64(9));
    }

    function testRejectsZeroStartingSequence() public {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        action.sequence = 0;
        vm.expectPartialRevert(ShotRegistry.InvalidSequence.selector);
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testFuzzCallerNeverBecomesOwner(address relayer) public {
        vm.assume(relayer != address(0));
        vm.assume(relayer != address(account));
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        setP256Expected(registry.hashCreateShot(action));
        vm.prank(relayer);
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
    }

    function testRelayerSubstitutionDoesNotChangeDigestOrAuthority() public {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        bytes32 digest = registry.hashCreateShot(action);
        setP256Expected(digest);
        vm.prank(address(0x1111));
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.AppendEvolutionAction memory appendAction = _appendAction(HEAD_1, HEAD_2, 2, 1);
        setP256Expected(registry.hashAppendEvolution(appendAction));
        vm.prank(address(0x2222));
        registry.appendEvolution(appendAction, p256Signature(KEY1_X, KEY1_Y));
        assertEq(registry.controllerOf(SHOT_ID), address(account));
        assertEq(registry.headOf(SHOT_ID), HEAD_2);
    }

    function testAppendEvolutionUpdatesHeadAndSequence() public {
        _createDefaultShot();
        ShotRegistry.AppendEvolutionAction memory action = _appendAction(HEAD_1, HEAD_2, 2, 1);
        setP256Expected(registry.hashAppendEvolution(action));
        registry.appendEvolution(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.head, HEAD_2);
        assertEq(shot.sequence, uint64(2));
        assertEq(shot.nonce, uint64(2));
        assertEq(shot.contentCommitment, CONTENT_1);
    }

    function testRejectsStalePreviousHead() public {
        _createDefaultShot();
        ShotRegistry.AppendEvolutionAction memory action = _appendAction(keccak256("stale"), HEAD_2, 2, 1);
        vm.expectPartialRevert(ShotRegistry.StaleHead.selector);
        registry.appendEvolution(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsSkippedSequence() public {
        _createDefaultShot();
        ShotRegistry.AppendEvolutionAction memory action = _appendAction(HEAD_1, HEAD_2, 3, 1);
        vm.expectPartialRevert(ShotRegistry.InvalidSequence.selector);
        registry.appendEvolution(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsReplayByNonce() public {
        _createDefaultShot();
        ShotRegistry.SetPublicStateAction memory action = ShotRegistry.SetPublicStateAction({
            shotId: SHOT_ID,
            currentHead: HEAD_1,
            sequence: 1,
            publicState: registry.PUBLIC_STATE_PUBLISHED(),
            contentCommitment: CONTENT_1,
            nonce: 1,
            deadline: _deadline()
        });
        setP256Expected(registry.hashSetPublicState(action));
        registry.setPublicState(action, p256Signature(KEY1_X, KEY1_Y));

        vm.expectPartialRevert(ShotRegistry.InvalidNonce.selector);
        registry.setPublicState(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsUnauthorizedAndHighSSignatures() public {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        setP256Expected(registry.hashCreateShot(action));
        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.createShot(action, p256Signature(KEY2_X, KEY2_Y));

        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.createShot(action, p256SignatureWithS(KEY1_X, KEY1_Y, P256_HALF_N + 1));
    }

    function testRejectsWrongRegistryDomain() public {
        ShotRegistry otherRegistry = new ShotRegistry();
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        setP256Expected(otherRegistry.hashCreateShot(action));

        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsWrongChainDomain() public {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        bytes32 digest = registry.hashCreateShot(action);
        setP256Expected(digest);
        vm.chainId(block.chainid + 1);

        vm.expectRevert(ShotRegistry.Unauthorized.selector);
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsExpiredAction() public {
        vm.warp(100);
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        action.deadline = 99;
        vm.expectPartialRevert(ShotRegistry.Expired.selector);
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testTransferChangesOnlyController() public {
        _createDefaultShot();
        ShotRegistry.TransferShotAction memory action = ShotRegistry.TransferShotAction({
            shotId: SHOT_ID,
            currentController: address(account),
            newController: address(nextAccount),
            currentHead: HEAD_1,
            sequence: 1,
            nonce: 1,
            deadline: _deadline()
        });
        setP256Expected(registry.hashTransferShot(action));
        vm.prank(RELAYER);
        registry.transferShot(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.Shot memory shot = registry.getShot(SHOT_ID);
        assertEq(shot.controller, address(nextAccount));
        assertEq(shot.head, HEAD_1);
        assertEq(shot.sequence, uint64(1));
        assertEq(shot.nonce, uint64(2));
        assertTrue(shot.controller != RELAYER);

        ShotRegistry.AppendEvolutionAction memory appendAction = _appendAction(HEAD_1, HEAD_2, 2, 2);
        setP256Expected(registry.hashAppendEvolution(appendAction));
        registry.appendEvolution(appendAction, p256Signature(KEY2_X, KEY2_Y));
        assertEq(registry.headOf(SHOT_ID), HEAD_2);
    }

    function testCreationRejectsLocalAndPrematureAppStoreStates() public {
        ShotRegistry.CreateShotAction memory localAction = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        localAction.publicState = 0;
        vm.expectPartialRevert(ShotRegistry.InvalidPublicState.selector);
        registry.createShot(localAction, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.CreateShotAction memory appStoreAction = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        appStoreAction.publicState = registry.PUBLIC_STATE_APP_STORE();
        vm.expectPartialRevert(ShotRegistry.InvalidPublicState.selector);
        registry.createShot(appStoreAction, p256Signature(KEY1_X, KEY1_Y));
    }

    function testPublicStateCannotDowngradeFromAppStore() public {
        _createDefaultShot();
        ShotRegistry.SetPublicStateAction memory graduateAction = ShotRegistry.SetPublicStateAction({
            shotId: SHOT_ID,
            currentHead: HEAD_1,
            sequence: 1,
            publicState: registry.PUBLIC_STATE_APP_STORE(),
            contentCommitment: CONTENT_1,
            nonce: 1,
            deadline: _deadline()
        });
        setP256Expected(registry.hashSetPublicState(graduateAction));
        registry.setPublicState(graduateAction, p256Signature(KEY1_X, KEY1_Y));

        ShotRegistry.SetPublicStateAction memory downgradeAction = ShotRegistry.SetPublicStateAction({
            shotId: SHOT_ID,
            currentHead: HEAD_1,
            sequence: 1,
            publicState: registry.PUBLIC_STATE_PUBLISHED(),
            contentCommitment: CONTENT_1,
            nonce: 2,
            deadline: _deadline()
        });
        vm.expectPartialRevert(ShotRegistry.InvalidPublicStateTransition.selector);
        registry.setPublicState(downgradeAction, p256Signature(KEY1_X, KEY1_Y));
    }

    function _createDefaultShot() private {
        ShotRegistry.CreateShotAction memory action = _createAction(SHOT_ID, address(account), HEAD_1, 0);
        setP256Expected(registry.hashCreateShot(action));
        registry.createShot(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function _createAction(bytes32 shotId, address controller, bytes32 head, uint64 nonce)
        private
        view
        returns (ShotRegistry.CreateShotAction memory)
    {
        return ShotRegistry.CreateShotAction({
            shotId: shotId,
            controller: controller,
            head: head,
            sequence: 1,
            publicState: registry.PUBLIC_STATE_PUBLISHED(),
            contentCommitment: CONTENT_1,
            nonce: nonce,
            deadline: _deadline()
        });
    }

    function _appendAction(bytes32 previousHead, bytes32 newHead, uint64 sequence, uint64 nonce)
        private
        view
        returns (ShotRegistry.AppendEvolutionAction memory)
    {
        return ShotRegistry.AppendEvolutionAction({
            shotId: SHOT_ID,
            previousHead: previousHead,
            newHead: newHead,
            sequence: sequence,
            contentCommitment: CONTENT_1,
            nonce: nonce,
            deadline: _deadline()
        });
    }

    function _deadline() private view returns (uint64) {
        return uint64(block.timestamp + 1 days);
    }
}
