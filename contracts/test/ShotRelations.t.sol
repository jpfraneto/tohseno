// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";
import {ShotRelations} from "../src/ShotRelations.sol";

contract ShotRelationsTest is ProtocolTestBase {
    bytes32 private constant SHOT_1 = keccak256("shot-1");
    bytes32 private constant SHOT_2 = keccak256("shot-2");
    bytes32 private constant HEAD_1 = keccak256("head-1");
    bytes32 private constant HEAD_2 = keccak256("head-2");
    address private constant TOKEN = address(0x70c);
    address private constant BASE_TOKEN_2 = address(0x70d);

    BuilderAccount private account1;
    BuilderAccount private account2;
    ShotRegistry private registry;
    ShotRelations private relations;

    function setUp() public {
        installP256Mock();
        account1 = newAccount(KEY1_X, KEY1_Y);
        account2 = newAccount(KEY2_X, KEY2_Y);
        registry = new ShotRegistry();
        _createShot(SHOT_1, HEAD_1, account1, KEY1_X, KEY1_Y);
        _createShot(SHOT_2, HEAD_2, account2, KEY2_X, KEY2_Y);
        relations = new ShotRelations(address(registry));
    }

    function testClaimsAndReleasesCanonicalHandle() public {
        string memory handle = "tohseno";
        bytes32 handleHash = keccak256(bytes(handle));
        ShotRelations.ClaimHandleAction memory claimAction =
            ShotRelations.ClaimHandleAction({shotId: SHOT_1, handleHash: handleHash, nonce: 0, deadline: _deadline()});
        setP256Expected(relations.hashClaimHandle(claimAction));
        vm.prank(address(0xaaa));
        relations.claimHandle(claimAction, handle, p256Signature(KEY1_X, KEY1_Y));

        assertEq(relations.shotByHandle(handleHash), SHOT_1);
        assertEq(relations.handleByShot(SHOT_1), handleHash);
        assertEq(relations.handleText(SHOT_1), handle);

        ShotRelations.ReleaseHandleAction memory releaseAction =
            ShotRelations.ReleaseHandleAction({shotId: SHOT_1, handleHash: handleHash, nonce: 1, deadline: _deadline()});
        setP256Expected(relations.hashReleaseHandle(releaseAction));
        relations.releaseHandle(releaseAction, p256Signature(KEY1_X, KEY1_Y));
        assertEq(relations.shotByHandle(handleHash), bytes32(0));
        assertEq(relations.handleByShot(SHOT_1), bytes32(0));
    }

    function testRejectsHandleCollision() public {
        string memory handle = "tohseno";
        bytes32 handleHash = keccak256(bytes(handle));
        _claimHandle(SHOT_1, handle, account1, KEY1_X, KEY1_Y);

        ShotRelations.ClaimHandleAction memory action =
            ShotRelations.ClaimHandleAction({shotId: SHOT_2, handleHash: handleHash, nonce: 0, deadline: _deadline()});
        vm.expectPartialRevert(ShotRelations.HandleCollision.selector);
        relations.claimHandle(action, handle, p256Signature(KEY2_X, KEY2_Y));
    }

    function testRejectsNonCanonicalHandle() public {
        string memory handle = "Tohseno";
        ShotRelations.ClaimHandleAction memory action = ShotRelations.ClaimHandleAction({
            shotId: SHOT_1,
            handleHash: keccak256(bytes(handle)),
            nonce: 0,
            deadline: _deadline()
        });
        vm.expectRevert(ShotRelations.InvalidHandle.selector);
        relations.claimHandle(action, handle, p256Signature(KEY1_X, KEY1_Y));
    }

    function testAssociatesAndRemovesExplicitAppcoin() public {
        ShotRelations.AssociateAppcoinAction memory associateAction = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 4663,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAssociateAppcoin(associateAction));
        vm.prank(address(0xbbb));
        relations.associateAppcoin(associateAction, p256Signature(KEY1_X, KEY1_Y));

        ShotRelations.Appcoin memory relation = relations.appcoinOf(SHOT_1);
        assertEq(relation.chainId, 4663);
        assertEq(relation.token, TOKEN);

        ShotRelations.RemoveAppcoinAction memory removeAction = ShotRelations.RemoveAppcoinAction({
            shotId: SHOT_1,
            chainId: 4663,
            token: TOKEN,
            nonce: 1,
            deadline: _deadline()
        });
        setP256Expected(relations.hashRemoveAppcoin(removeAction));
        relations.removeAppcoin(removeAction, p256Signature(KEY1_X, KEY1_Y));
        relation = relations.appcoinOf(SHOT_1);
        assertEq(relation.chainId, 0);
        assertEq(relation.token, address(0));
    }

    function testBaseAssociationFreshNonceReplacementAndExactRemovalSemantics() public {
        ShotRelations.AssociateAppcoinAction memory initial = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 8453,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAssociateAppcoin(initial));
        relations.associateAppcoin(initial, p256Signature(KEY1_X, KEY1_Y));
        assertEq(relations.appcoinOf(SHOT_1).chainId, 8453);
        assertEq(relations.appcoinOf(SHOT_1).token, TOKEN);

        // The deployed-candidate ABI intentionally uses one current slot. An
        // identical association with a fresh nonce is a valid replacement.
        initial.nonce = 1;
        setP256Expected(relations.hashAssociateAppcoin(initial));
        relations.associateAppcoin(initial, p256Signature(KEY1_X, KEY1_Y));

        // A conflicting fresh authorized value also replaces the current slot;
        // event history, rather than storage, preserves earlier associations.
        ShotRelations.AssociateAppcoinAction memory replacement = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 8453,
            token: BASE_TOKEN_2,
            nonce: 2,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAssociateAppcoin(replacement));
        relations.associateAppcoin(replacement, p256Signature(KEY1_X, KEY1_Y));
        assertEq(relations.appcoinOf(SHOT_1).chainId, 8453);
        assertEq(relations.appcoinOf(SHOT_1).token, BASE_TOKEN_2);

        ShotRelations.RemoveAppcoinAction memory mismatch = ShotRelations.RemoveAppcoinAction({
            shotId: SHOT_1,
            chainId: 8453,
            token: TOKEN,
            nonce: 3,
            deadline: _deadline()
        });
        vm.expectRevert(ShotRelations.AppcoinMismatch.selector);
        relations.removeAppcoin(mismatch, p256Signature(KEY1_X, KEY1_Y));

        ShotRelations.RemoveAppcoinAction memory exact = ShotRelations.RemoveAppcoinAction({
            shotId: SHOT_1,
            chainId: 8453,
            token: BASE_TOKEN_2,
            nonce: 3,
            deadline: _deadline()
        });
        setP256Expected(relations.hashRemoveAppcoin(exact));
        relations.removeAppcoin(exact, p256Signature(KEY1_X, KEY1_Y));
        assertEq(relations.appcoinOf(SHOT_1).chainId, 0);
        assertEq(relations.appcoinOf(SHOT_1).token, address(0));

        vm.expectPartialRevert(ShotRelations.InvalidNonce.selector);
        relations.removeAppcoin(exact, p256Signature(KEY1_X, KEY1_Y));
    }

    function testRejectsZeroChainOrTokenWithoutConsumingNonce() public {
        ShotRelations.AssociateAppcoinAction memory action = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 0,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        vm.expectRevert(ShotRelations.InvalidAppcoin.selector);
        relations.associateAppcoin(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(relations.nonces(SHOT_1), 0);

        action.chainId = 8453;
        action.token = address(0);
        vm.expectRevert(ShotRelations.InvalidAppcoin.selector);
        relations.associateAppcoin(action, p256Signature(KEY1_X, KEY1_Y));
        assertEq(relations.nonces(SHOT_1), 0);
    }

    function testRejectsRemovingMissingAppcoin() public {
        ShotRelations.RemoveAppcoinAction memory action = ShotRelations.RemoveAppcoinAction({
            shotId: SHOT_1,
            chainId: 4663,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        vm.expectRevert(ShotRelations.AppcoinMismatch.selector);
        relations.removeAppcoin(action, p256Signature(KEY1_X, KEY1_Y));
    }

    function testUnauthorizedDeviceCannotAssociateAppcoin() public {
        ShotRelations.AssociateAppcoinAction memory action = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 4663,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAssociateAppcoin(action));
        vm.expectRevert(ShotRelations.Unauthorized.selector);
        relations.associateAppcoin(action, p256Signature(KEY2_X, KEY2_Y));
    }

    function testAppStoreAttestationMustNameCurrentEvolution() public {
        ShotRelations.AttestAppStoreAction memory staleAction = ShotRelations.AttestAppStoreAction({
            shotId: SHOT_1,
            bundleIdHash: keccak256("com.tohseno.genesis"),
            storeId: 123,
            evolutionHead: HEAD_2,
            nonce: 0,
            deadline: _deadline()
        });
        vm.expectPartialRevert(ShotRelations.StaleEvolution.selector);
        relations.attestAppStore(staleAction, p256Signature(KEY1_X, KEY1_Y));

        ShotRelations.AttestAppStoreAction memory action = ShotRelations.AttestAppStoreAction({
            shotId: SHOT_1,
            bundleIdHash: keccak256("com.tohseno.genesis"),
            storeId: 123,
            evolutionHead: HEAD_1,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAttestAppStore(action));
        relations.attestAppStore(action, p256Signature(KEY1_X, KEY1_Y));
        ShotRelations.AppStoreAttestation memory attestation = relations.appStoreAttestationOf(SHOT_1);
        assertEq(attestation.bundleIdHash, action.bundleIdHash);
        assertEq(attestation.storeId, uint64(123));
        assertEq(attestation.evolutionHead, HEAD_1);
    }

    function testTransferredControllerImmediatelyOwnsRelations() public {
        ShotRegistry.TransferShotAction memory transferAction = ShotRegistry.TransferShotAction({
            shotId: SHOT_1,
            currentController: address(account1),
            newController: address(account2),
            currentHead: HEAD_1,
            sequence: 1,
            nonce: 1,
            deadline: _deadline()
        });
        setP256Expected(registry.hashTransferShot(transferAction));
        registry.transferShot(transferAction, p256Signature(KEY1_X, KEY1_Y));

        ShotRelations.AssociateAppcoinAction memory relationAction = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 4663,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAssociateAppcoin(relationAction));
        vm.expectRevert(ShotRelations.Unauthorized.selector);
        relations.associateAppcoin(relationAction, p256Signature(KEY1_X, KEY1_Y));

        relations.associateAppcoin(relationAction, p256Signature(KEY2_X, KEY2_Y));
        assertEq(relations.appcoinOf(SHOT_1).token, TOKEN);
    }

    function testRejectsReplayWrongDomainAndExpiry() public {
        ShotRelations.AssociateAppcoinAction memory action = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_1,
            chainId: 4663,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(relations.hashAssociateAppcoin(action));
        relations.associateAppcoin(action, p256Signature(KEY1_X, KEY1_Y));
        vm.expectPartialRevert(ShotRelations.InvalidNonce.selector);
        relations.associateAppcoin(action, p256Signature(KEY1_X, KEY1_Y));

        ShotRelations other = new ShotRelations(address(registry));
        ShotRelations.AssociateAppcoinAction memory shot2Action = ShotRelations.AssociateAppcoinAction({
            shotId: SHOT_2,
            chainId: 4663,
            token: TOKEN,
            nonce: 0,
            deadline: _deadline()
        });
        setP256Expected(other.hashAssociateAppcoin(shot2Action));
        vm.expectRevert(ShotRelations.Unauthorized.selector);
        relations.associateAppcoin(shot2Action, p256Signature(KEY2_X, KEY2_Y));

        vm.warp(100);
        shot2Action.deadline = 99;
        vm.expectPartialRevert(ShotRelations.Expired.selector);
        relations.associateAppcoin(shot2Action, p256Signature(KEY2_X, KEY2_Y));
    }

    function _claimHandle(bytes32 shotId, string memory handle, BuilderAccount, uint256 signerX, uint256 signerY)
        private
    {
        ShotRelations.ClaimHandleAction memory action = ShotRelations.ClaimHandleAction({
            shotId: shotId,
            handleHash: keccak256(bytes(handle)),
            nonce: relations.nonces(shotId),
            deadline: _deadline()
        });
        setP256Expected(relations.hashClaimHandle(action));
        relations.claimHandle(action, handle, p256Signature(signerX, signerY));
    }

    function _createShot(bytes32 shotId, bytes32 head, BuilderAccount controller, uint256 signerX, uint256 signerY)
        private
    {
        ShotRegistry.CreateShotAction memory action = ShotRegistry.CreateShotAction({
            shotId: shotId,
            controller: address(controller),
            head: head,
            sequence: 1,
            publicState: registry.PUBLIC_STATE_PUBLISHED(),
            contentCommitment: keccak256(abi.encodePacked(shotId, head)),
            nonce: registry.createNonces(address(controller)),
            deadline: _deadline()
        });
        setP256Expected(registry.hashCreateShot(action));
        registry.createShot(action, p256Signature(signerX, signerY));
    }

    function _deadline() private view returns (uint64) {
        return uint64(block.timestamp + 1 days);
    }
}
