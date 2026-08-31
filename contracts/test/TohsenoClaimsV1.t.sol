// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";
import {TohsenoClaimsV1} from "../src/TohsenoClaimsV1.sol";

contract TohsenoClaimsV1Test is ProtocolTestBase {
    bytes32 private constant SHOT_ID = keccak256("claims-shot-1");
    bytes32 private constant SHOT_ID_2 = keccak256("claims-shot-2");
    bytes32 private constant FORK_SHOT_ID = keccak256("claims-fork-shot");
    bytes32 private constant HEAD_1 = keccak256("claims-head-1");
    bytes32 private constant HEAD_2 = keccak256("claims-head-2");
    bytes32 private constant RELEASE_1 = keccak256("claims-release-1");
    bytes32 private constant RELEASE_2 = keccak256("claims-release-2");
    bytes32 private constant GESTURE_1 = keccak256("claims-gesture-1");
    bytes32 private constant GESTURE_2 = keccak256("claims-gesture-2");

    BuilderAccount private controller;
    BuilderAccount private claimantA;
    BuilderAccount private claimantB;
    BuilderAccount private claimantC;
    BuilderAccount private nextController;
    ShotRegistry private registry;
    TohsenoClaimsV1 private claims;

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

    function setUp() public {
        installP256Mock();
        controller = newAccount(KEY1_X, KEY1_Y);
        claimantA = newAccount(KEY2_X, KEY2_Y);
        claimantB = newAccount(KEY3_X, KEY3_Y);
        claimantC = newAccount(KEY1_X, KEY1_Y);
        nextController = newAccount(KEY3_X, KEY3_Y);
        registry = new ShotRegistry();
        claims = new TohsenoClaimsV1(address(registry));
        _register(SHOT_ID, address(controller), HEAD_1, KEY1_X, KEY1_Y);
    }

    function testConstructorRequiresDeployedRegistry() public {
        vm.expectRevert(TohsenoClaimsV1.InvalidShotRegistry.selector);
        new TohsenoClaimsV1(address(0));

        vm.expectRevert(TohsenoClaimsV1.InvalidShotRegistry.selector);
        new TohsenoClaimsV1(address(0x1234));
    }

    function testFrozenRustSwiftSolidityActionVectors() public {
        vm.chainId(4663);
        assertEq(
            claims.OPEN_CLAIM_EDITION_TYPEHASH(), 0xd1daec7e8aa22a0cf52d8bd6abe004e8bdabe1639512e277f0649dbd561264de
        );
        assertEq(claims.CLAIM_SOFTWARE_TYPEHASH(), 0x71009b002c8d419124eeba3203fcc83c96279ad6763d6ba1438f59a8a3173fb9);
        address fixedAddress = 0x6666666666666666666666666666666666666666;
        vm.etch(fixedAddress, address(claims).code);
        TohsenoClaimsV1 fixedClaims = TohsenoClaimsV1(fixedAddress);
        assertEq(fixedClaims.domainSeparator(), 0xe1ab55a103d9cb852de025d63b463b0e140ecfd82cadbe0ad74ad1d19c16d087);
        TohsenoClaimsV1.OpenClaimEditionAction memory open = TohsenoClaimsV1.OpenClaimEditionAction({
            shotRegistry: 0x3FE6508Ba2660Bc575080024F402C192A2E035A0,
            shotId: bytes32(uint256(0x1111111111111111111111111111111111111111111111111111111111111111)),
            maxClaims: 888,
            closesAt: 2_000_000_000,
            controller: 0x2222222222222222222222222222222222222222,
            nonce: 7,
            deadline: 2_000_000_100
        });
        assertEq(
            fixedClaims.hashOpenClaimEdition(open), 0x7bef08336b81593e85bf52432fab9c5fa4da0648f6ae24a8944b686320b5f258
        );
        TohsenoClaimsV1.ClaimSoftwareAction memory claim = TohsenoClaimsV1.ClaimSoftwareAction({
            shotRegistry: 0x3FE6508Ba2660Bc575080024F402C192A2E035A0,
            shotId: bytes32(uint256(0x1111111111111111111111111111111111111111111111111111111111111111)),
            claimant: 0x4444444444444444444444444444444444444444,
            releaseDigest: bytes32(uint256(0x5555555555555555555555555555555555555555555555555555555555555555)),
            checkpointDigest: bytes32(uint256(0x7777777777777777777777777777777777777777777777777777777777777777)),
            gestureCommitment: 0x23ff9441e61d47a40c542827940bf16cf1f96311e8435c0b8920e97e97861e87,
            nonce: 9,
            deadline: 2_000_000_100
        });
        assertEq(
            fixedClaims.hashClaimSoftware(claim), 0x43375f1f8e8b643e697f5365c3c1ccb989f4d6251c839b0699375d1143ee9fb3
        );
    }

    function testOpenEditionForRegisteredShotUsesExactCurrentControllerAuthorization() public {
        uint64 now64 = _now64();
        TohsenoClaimsV1.OpenClaimEditionAction memory action = _openAction(SHOT_ID, 888, now64 + 1 days);
        setP256Expected(claims.hashOpenClaimEdition(action));

        vm.expectEmit(true, true, false, true, address(claims));
        emit ClaimEditionOpened(SHOT_ID, address(controller), 888, now64, now64 + 1 days);
        vm.prank(address(0xbeef));
        claims.openClaimEdition(action, p256Signature(KEY1_X, KEY1_Y));

        TohsenoClaimsV1.ClaimEdition memory edition = claims.claimEdition(SHOT_ID);
        assertTrue(edition.opened);
        assertEq(edition.maxClaims, uint64(888));
        assertEq(edition.totalClaims, uint64(0));
        assertEq(edition.openedAt, now64);
        assertEq(edition.closesAt, now64 + 1 days);
        assertEq(claims.editionNonces(address(controller)), uint64(1));
    }

    function testRejectsUnregisteredShotWrongControllerAndWrongRegistry() public {
        TohsenoClaimsV1.OpenClaimEditionAction memory missing = _openAction(SHOT_ID_2, 0, 0);
        vm.expectPartialRevert(TohsenoClaimsV1.ShotNotFound.selector);
        claims.openClaimEdition(missing, hex"");

        TohsenoClaimsV1.OpenClaimEditionAction memory wrongController = _openAction(SHOT_ID, 0, 0);
        wrongController.controller = address(nextController);
        vm.expectPartialRevert(TohsenoClaimsV1.InvalidController.selector);
        claims.openClaimEdition(wrongController, hex"");

        TohsenoClaimsV1.OpenClaimEditionAction memory wrongRegistry = _openAction(SHOT_ID, 0, 0);
        wrongRegistry.shotRegistry = address(controller);
        vm.expectRevert(TohsenoClaimsV1.InvalidShotRegistry.selector);
        claims.openClaimEdition(wrongRegistry, hex"");
    }

    function testRejectsExpiredInvalidPolicyAndInvalidControllerSignature() public {
        TohsenoClaimsV1.OpenClaimEditionAction memory expired = _openAction(SHOT_ID, 0, 0);
        expired.deadline = _now64() - 1;
        vm.expectPartialRevert(TohsenoClaimsV1.Expired.selector);
        claims.openClaimEdition(expired, hex"");

        TohsenoClaimsV1.OpenClaimEditionAction memory closed = _openAction(SHOT_ID, 0, _now64());
        vm.expectRevert(TohsenoClaimsV1.InvalidEditionPolicy.selector);
        claims.openClaimEdition(closed, hex"");

        TohsenoClaimsV1.OpenClaimEditionAction memory unauthorized = _openAction(SHOT_ID, 0, 0);
        setP256Expected(claims.hashOpenClaimEdition(unauthorized));
        vm.expectRevert(TohsenoClaimsV1.Unauthorized.selector);
        claims.openClaimEdition(unauthorized, p256Signature(KEY2_X, KEY2_Y));
    }

    function testEditionNonceReplayAndSecondOpeningAreRejected() public {
        _register(SHOT_ID_2, address(controller), HEAD_1, KEY1_X, KEY1_Y);
        TohsenoClaimsV1.OpenClaimEditionAction memory first = _openAction(SHOT_ID, 0, 0);
        _open(first, KEY1_X, KEY1_Y);

        TohsenoClaimsV1.OpenClaimEditionAction memory replay = _openAction(SHOT_ID_2, 0, 0);
        replay.nonce = 0;
        vm.expectPartialRevert(TohsenoClaimsV1.InvalidNonce.selector);
        claims.openClaimEdition(replay, hex"");

        TohsenoClaimsV1.OpenClaimEditionAction memory duplicate = _openAction(SHOT_ID, 99, 0);
        vm.expectPartialRevert(TohsenoClaimsV1.EditionAlreadyOpened.selector);
        claims.openClaimEdition(duplicate, hex"");

        TohsenoClaimsV1.ClaimEdition memory edition = claims.claimEdition(SHOT_ID);
        assertEq(edition.maxClaims, uint64(0));
        assertEq(edition.closesAt, uint64(0));
    }

    function testSupportsExactlyFourEditionPolicyShapes() public {
        _register(SHOT_ID_2, address(controller), HEAD_1, KEY1_X, KEY1_Y);
        _register(FORK_SHOT_ID, address(nextController), HEAD_1, KEY3_X, KEY3_Y);
        bytes32 fourthShot = keccak256("claims-fourth-shot");
        _register(fourthShot, address(nextController), HEAD_1, KEY3_X, KEY3_Y);

        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);
        _open(_openAction(SHOT_ID_2, 888, 0), KEY1_X, KEY1_Y);
        _open(_openAction(FORK_SHOT_ID, 0, _now64() + 1 days), KEY3_X, KEY3_Y);
        _open(_openAction(fourthShot, 888, _now64() + 1 days), KEY3_X, KEY3_Y);

        TohsenoClaimsV1.ClaimEdition memory open = claims.claimEdition(SHOT_ID);
        TohsenoClaimsV1.ClaimEdition memory limited = claims.claimEdition(SHOT_ID_2);
        TohsenoClaimsV1.ClaimEdition memory timed = claims.claimEdition(FORK_SHOT_ID);
        TohsenoClaimsV1.ClaimEdition memory both = claims.claimEdition(fourthShot);
        assertEq(open.maxClaims, uint64(0));
        assertEq(open.closesAt, uint64(0));
        assertEq(limited.maxClaims, uint64(888));
        assertEq(limited.closesAt, uint64(0));
        assertEq(timed.maxClaims, uint64(0));
        assertTrue(timed.closesAt != 0);
        assertEq(both.maxClaims, uint64(888));
        assertTrue(both.closesAt != 0);
    }

    function testOpenEditionMintsUniqueLockedERC721ReceiptsWithPerShotNumbers() public {
        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);

        vm.expectEmit(true, true, true, true, address(claims));
        emit Transfer(address(0), address(claimantA), 1);
        vm.expectEmit(true, false, false, true, address(claims));
        emit Locked(1);
        vm.expectEmit(true, true, true, true, address(claims));
        emit SoftwareClaimed(SHOT_ID, address(claimantA), 1, 1, RELEASE_1, HEAD_1, GESTURE_1);
        uint256 first = _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        uint256 second = _claim(claimantB, KEY3_X, KEY3_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);

        assertEq(first, uint256(1));
        assertEq(second, uint256(2));
        assertEq(claims.nextTokenId(), uint256(3));
        assertEq(claims.ownerOf(first), address(claimantA));
        assertEq(claims.ownerOf(second), address(claimantB));
        assertEq(claims.balanceOf(address(claimantA)), uint256(1));
        assertEq(claims.claimTokenOf(SHOT_ID, address(claimantA)), first);
        assertEq(claims.claimTokenOf(SHOT_ID, address(claimantB)), second);
        assertTrue(claims.locked(first));
        assertEq(claims.tokenURI(first), "https://tohseno.com/api/claims/v1/token/1");

        TohsenoClaimsV1.ClaimRecord memory record = claims.claimRecord(first);
        assertEq(record.shotId, SHOT_ID);
        assertEq(record.claimNumber, uint64(1));
        assertEq(record.claimant, address(claimantA));
        assertEq(record.releaseDigest, RELEASE_1);
        assertEq(record.checkpointDigest, HEAD_1);
        assertEq(record.gestureCommitment, GESTURE_1);
        TohsenoClaimsV1.ClaimEdition memory edition = claims.claimEdition(SHOT_ID);
        assertEq(edition.totalClaims, uint64(2));

        assertTrue(claims.supportsInterface(0x01ffc9a7));
        assertTrue(claims.supportsInterface(0x80ac58cd));
        assertTrue(claims.supportsInterface(0x5b5e139f));
        assertTrue(claims.supportsInterface(0xb45a3c0e));
        assertFalse(claims.supportsInterface(0xffffffff));
    }

    function testFiniteSupplyMintsExactlyNAndChainOrderingClosesEdition() public {
        _open(_openAction(SHOT_ID, 2, 0), KEY1_X, KEY1_Y);
        _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        _claim(claimantB, KEY3_X, KEY3_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);
        assertTrue(claims.editionIsClosed(SHOT_ID));

        TohsenoClaimsV1.ClaimSoftwareAction memory third =
            _claimAction(address(claimantC), SHOT_ID, RELEASE_1, HEAD_1, keccak256("third"));
        vm.expectPartialRevert(TohsenoClaimsV1.SupplyExhausted.selector);
        claims.claimSoftware(third, p256Signature(KEY1_X, KEY1_Y));

        TohsenoClaimsV1.ClaimEdition memory edition = claims.claimEdition(SHOT_ID);
        assertEq(edition.totalClaims, uint64(2));
        assertEq(claims.nextTokenId(), uint256(3));
    }

    function testTimedEditionAcceptsBeforeButRejectsAtExactCloseBoundary() public {
        uint64 closesAt = _now64() + 60;
        _open(_openAction(SHOT_ID, 0, closesAt), KEY1_X, KEY1_Y);
        vm.warp(uint256(closesAt) - 1);
        _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        vm.warp(closesAt);
        assertTrue(claims.editionIsClosed(SHOT_ID));

        TohsenoClaimsV1.ClaimSoftwareAction memory second =
            _claimAction(address(claimantB), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);
        vm.expectPartialRevert(TohsenoClaimsV1.EditionClosed.selector);
        claims.claimSoftware(second, p256Signature(KEY3_X, KEY3_Y));
    }

    function testLimitedTimedEditionClosesAtWhicheverBoundaryOccursFirst() public {
        uint64 closesAt = _now64() + 1 days;
        _open(_openAction(SHOT_ID, 1, closesAt), KEY1_X, KEY1_Y);
        _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        assertTrue(claims.editionIsClosed(SHOT_ID));

        TohsenoClaimsV1.ClaimSoftwareAction memory second =
            _claimAction(address(claimantB), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);
        vm.expectPartialRevert(TohsenoClaimsV1.SupplyExhausted.selector);
        claims.claimSoftware(second, p256Signature(KEY3_X, KEY3_Y));
    }

    function testOneAccountCanClaimShotOnlyOnceForever() public {
        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);
        uint256 first = _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        TohsenoClaimsV1.ClaimSoftwareAction memory duplicate =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);
        vm.expectPartialRevert(TohsenoClaimsV1.AlreadyClaimed.selector);
        claims.claimSoftware(duplicate, p256Signature(KEY2_X, KEY2_Y));
        assertEq(claims.claimTokenOf(SHOT_ID, address(claimantA)), first);
    }

    function testClaimRejectsMissingFieldsEOAStaleHeadAndUnopenedEdition() public {
        TohsenoClaimsV1.ClaimSoftwareAction memory unopened =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        vm.expectPartialRevert(TohsenoClaimsV1.EditionNotOpened.selector);
        claims.claimSoftware(unopened, hex"");
        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);

        TohsenoClaimsV1.ClaimSoftwareAction memory eoa =
            _claimAction(address(0xa11ce), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        vm.expectRevert(TohsenoClaimsV1.InvalidClaimant.selector);
        claims.claimSoftware(eoa, hex"");

        TohsenoClaimsV1.ClaimSoftwareAction memory zeroRelease =
            _claimAction(address(claimantA), SHOT_ID, bytes32(0), HEAD_1, GESTURE_1);
        vm.expectRevert(TohsenoClaimsV1.InvalidReleaseDigest.selector);
        claims.claimSoftware(zeroRelease, hex"");

        TohsenoClaimsV1.ClaimSoftwareAction memory zeroHead =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, bytes32(0), GESTURE_1);
        vm.expectRevert(TohsenoClaimsV1.InvalidCheckpointDigest.selector);
        claims.claimSoftware(zeroHead, hex"");

        TohsenoClaimsV1.ClaimSoftwareAction memory zeroGesture =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, HEAD_1, bytes32(0));
        vm.expectRevert(TohsenoClaimsV1.InvalidGestureCommitment.selector);
        claims.claimSoftware(zeroGesture, hex"");

        TohsenoClaimsV1.ClaimSoftwareAction memory stale =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, HEAD_2, GESTURE_1);
        vm.expectPartialRevert(TohsenoClaimsV1.StaleCheckpoint.selector);
        claims.claimSoftware(stale, hex"");
    }

    function testClaimNonceDeadlineSignatureChainAndContractDomainAreExact() public {
        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);
        TohsenoClaimsV1.ClaimSoftwareAction memory action =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);

        action.nonce = 1;
        vm.expectPartialRevert(TohsenoClaimsV1.InvalidNonce.selector);
        claims.claimSoftware(action, hex"");
        action.nonce = 0;

        action.deadline = _now64() - 1;
        vm.expectPartialRevert(TohsenoClaimsV1.Expired.selector);
        claims.claimSoftware(action, hex"");
        action.deadline = _now64() + 1 days;

        action.shotRegistry = address(controller);
        vm.expectRevert(TohsenoClaimsV1.InvalidShotRegistry.selector);
        claims.claimSoftware(action, hex"");
        action.shotRegistry = address(registry);

        setP256Expected(claims.hashClaimSoftware(action));
        vm.expectRevert(TohsenoClaimsV1.Unauthorized.selector);
        claims.claimSoftware(action, p256Signature(KEY3_X, KEY3_Y));

        bytes32 oldChainDigest = claims.hashClaimSoftware(action);
        setP256Expected(oldChainDigest);
        vm.chainId(4664);
        vm.expectRevert(TohsenoClaimsV1.Unauthorized.selector);
        claims.claimSoftware(action, p256Signature(KEY2_X, KEY2_Y));
    }

    function testShotUpdatePreservesClaimsAndEditionWithoutMintingAgain() public {
        _open(_openAction(SHOT_ID, 888, 0), KEY1_X, KEY1_Y);
        uint256 first = _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        _append(SHOT_ID, HEAD_1, HEAD_2, controller, KEY1_X, KEY1_Y);

        TohsenoClaimsV1.ClaimEdition memory edition = claims.claimEdition(SHOT_ID);
        assertEq(edition.maxClaims, uint64(888));
        assertEq(edition.totalClaims, uint64(1));
        assertEq(claims.ownerOf(first), address(claimantA));
        TohsenoClaimsV1.ClaimRecord memory oldReceipt = claims.claimRecord(first);
        assertEq(oldReceipt.releaseDigest, RELEASE_1);
        assertEq(oldReceipt.checkpointDigest, HEAD_1);

        TohsenoClaimsV1.ClaimSoftwareAction memory stale =
            _claimAction(address(claimantB), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);
        vm.expectPartialRevert(TohsenoClaimsV1.StaleCheckpoint.selector);
        claims.claimSoftware(stale, hex"");

        uint256 second = _claim(claimantB, KEY3_X, KEY3_Y, SHOT_ID, RELEASE_2, HEAD_2, GESTURE_2);
        TohsenoClaimsV1.ClaimRecord memory newReceipt = claims.claimRecord(second);
        assertEq(newReceipt.claimNumber, uint64(2));
        assertEq(newReceipt.releaseDigest, RELEASE_2);
        assertEq(newReceipt.checkpointDigest, HEAD_2);
    }

    function testControllerTransferPreservesEditionAndExistingClaims() public {
        _open(_openAction(SHOT_ID, 2, 0), KEY1_X, KEY1_Y);
        uint256 first = _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        _transfer(SHOT_ID, controller, nextController, KEY1_X, KEY1_Y);

        TohsenoClaimsV1.ClaimEdition memory edition = claims.claimEdition(SHOT_ID);
        assertEq(edition.maxClaims, uint64(2));
        assertEq(edition.totalClaims, uint64(1));
        assertEq(claims.ownerOf(first), address(claimantA));
        _claim(claimantB, KEY3_X, KEY3_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_2);
        assertTrue(claims.editionIsClosed(SHOT_ID));
    }

    function testForkChildHasIndependentEditionNumbersAndSupply() public {
        _register(FORK_SHOT_ID, address(nextController), HEAD_1, KEY3_X, KEY3_Y);
        _open(_openAction(SHOT_ID, 1, 0), KEY1_X, KEY1_Y);
        _open(_openAction(FORK_SHOT_ID, 0, 0), KEY3_X, KEY3_Y);

        uint256 parent = _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        uint256 child = _claim(claimantA, KEY2_X, KEY2_Y, FORK_SHOT_ID, RELEASE_2, HEAD_1, GESTURE_2);
        assertEq(claims.claimRecord(parent).claimNumber, uint64(1));
        assertEq(claims.claimRecord(child).claimNumber, uint64(1));
        assertEq(claims.claimTokenOf(SHOT_ID, address(claimantA)), parent);
        assertEq(claims.claimTokenOf(FORK_SHOT_ID, address(claimantA)), child);
        assertEq(claims.balanceOf(address(claimantA)), uint256(2));
    }

    function testEveryApprovalAndTransferPathReverts() public {
        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);
        uint256 token = _claim(claimantA, KEY2_X, KEY2_Y, SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        assertEq(claims.getApproved(token), address(0));
        assertFalse(claims.isApprovedForAll(address(claimantA), address(0xbeef)));

        vm.expectRevert(TohsenoClaimsV1.NonTransferable.selector);
        claims.approve(address(0xbeef), token);
        vm.expectRevert(TohsenoClaimsV1.NonTransferable.selector);
        claims.setApprovalForAll(address(0xbeef), true);
        vm.expectRevert(TohsenoClaimsV1.NonTransferable.selector);
        claims.transferFrom(address(claimantA), address(claimantB), token);
        vm.expectRevert(TohsenoClaimsV1.NonTransferable.selector);
        claims.safeTransferFrom(address(claimantA), address(claimantB), token);
        vm.expectRevert(TohsenoClaimsV1.NonTransferable.selector);
        claims.safeTransferFrom(address(claimantA), address(claimantB), token, hex"cafe");

        assertEq(claims.ownerOf(token), address(claimantA));
        assertEq(claims.balanceOf(address(claimantA)), uint256(1));
    }

    function testMintNeedsNoERC721ReceiverCallbackAndHasBoundedGas() public {
        _open(_openAction(SHOT_ID, 0, 0), KEY1_X, KEY1_Y);
        TohsenoClaimsV1.ClaimSoftwareAction memory action =
            _claimAction(address(claimantA), SHOT_ID, RELEASE_1, HEAD_1, GESTURE_1);
        setP256Expected(claims.hashClaimSoftware(action));
        uint256 before = gasleft();
        uint256 token = claims.claimSoftware(action, p256Signature(KEY2_X, KEY2_Y));
        uint256 used = before - gasleft();
        assertEq(claims.ownerOf(token), address(claimantA));
        assertTrue(used < 500_000);
    }

    function _openAction(bytes32 shotId, uint64 maxClaims, uint64 closesAt)
        private
        view
        returns (TohsenoClaimsV1.OpenClaimEditionAction memory)
    {
        address currentController = registry.controllerOf(shotId);
        if (currentController == address(0)) currentController = address(controller);
        return TohsenoClaimsV1.OpenClaimEditionAction({
            shotRegistry: address(registry),
            shotId: shotId,
            maxClaims: maxClaims,
            closesAt: closesAt,
            controller: currentController,
            nonce: claims.editionNonces(currentController),
            deadline: _now64() + 1 days
        });
    }

    function _claimAction(
        address claimant,
        bytes32 shotId,
        bytes32 releaseDigest,
        bytes32 checkpointDigest,
        bytes32 gestureCommitment
    ) private view returns (TohsenoClaimsV1.ClaimSoftwareAction memory) {
        return TohsenoClaimsV1.ClaimSoftwareAction({
            shotRegistry: address(registry),
            shotId: shotId,
            claimant: claimant,
            releaseDigest: releaseDigest,
            checkpointDigest: checkpointDigest,
            gestureCommitment: gestureCommitment,
            nonce: claims.claimNonces(claimant),
            deadline: _now64() + 1 days
        });
    }

    function _open(TohsenoClaimsV1.OpenClaimEditionAction memory action, uint256 x, uint256 y) private {
        setP256Expected(claims.hashOpenClaimEdition(action));
        claims.openClaimEdition(action, p256Signature(x, y));
    }

    function _claim(
        BuilderAccount claimant,
        uint256 x,
        uint256 y,
        bytes32 shotId,
        bytes32 releaseDigest,
        bytes32 checkpointDigest,
        bytes32 gestureCommitment
    ) private returns (uint256) {
        TohsenoClaimsV1.ClaimSoftwareAction memory action =
            _claimAction(address(claimant), shotId, releaseDigest, checkpointDigest, gestureCommitment);
        setP256Expected(claims.hashClaimSoftware(action));
        return claims.claimSoftware(action, p256Signature(x, y));
    }

    function _register(bytes32 shotId, address shotController, bytes32 head, uint256 x, uint256 y) private {
        bytes32 salt = keccak256(abi.encode("claims-registration", shotId));
        ShotRegistry.RegisterShotAction memory action = ShotRegistry.RegisterShotAction({
            shotId: shotId,
            controller: shotController,
            head: head,
            salt: salt,
            nonce: registry.registrationNonces(shotController),
            deadline: _now64() + 1 days
        });
        registry.commitShot(registry.registrationCommitment(shotController, shotId, salt, action.deadline));
        vm.warp(block.timestamp + registry.MIN_COMMIT_AGE());
        setP256Expected(registry.hashRegisterShot(action));
        registry.registerShot(action, p256Signature(x, y));
    }

    function _append(
        bytes32 shotId,
        bytes32 previousHead,
        bytes32 newHead,
        BuilderAccount shotController,
        uint256 x,
        uint256 y
    ) private {
        ShotRegistry.AppendCheckpointAction memory action = ShotRegistry.AppendCheckpointAction({
            shotId: shotId,
            previousHead: previousHead,
            newHead: newHead,
            checkpointSequence: registry.checkpointSequenceOf(shotId) + 1,
            nonce: registry.nonceOf(shotId),
            deadline: _now64() + 1 days
        });
        setP256Expected(registry.hashAppendCheckpoint(action));
        registry.appendCheckpoint(action, p256Signature(x, y));
        assertEq(registry.controllerOf(shotId), address(shotController));
    }

    function _transfer(bytes32 shotId, BuilderAccount from, BuilderAccount to, uint256 x, uint256 y) private {
        ShotRegistry.TransferShotAction memory action = ShotRegistry.TransferShotAction({
            shotId: shotId,
            currentController: address(from),
            newController: address(to),
            currentHead: registry.headOf(shotId),
            checkpointSequence: registry.checkpointSequenceOf(shotId),
            nonce: registry.nonceOf(shotId),
            deadline: _now64() + 1 days
        });
        setP256Expected(registry.hashTransferShot(action));
        registry.transferShot(action, p256Signature(x, y));
    }

    function _now64() private view returns (uint64) {
        return uint64(block.timestamp);
    }
}
