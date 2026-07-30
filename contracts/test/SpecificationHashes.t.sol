// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";

contract SpecificationHashesTest is ProtocolTestBase {
    bytes32 private constant DOMAIN_TYPEHASH = 0x8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f;

    function testTypeHashesAreTheDocumentedProtocolConstants() public {
        BuilderAccount account = newAccount(KEY1_X, KEY1_Y);
        ShotRegistry registry = new ShotRegistry();

        assertEq(
            account.AUTHORIZE_DEVICE_TYPEHASH(), 0xcdb0126f85b19b28642fc350e8d771c41afd39854da572f865a3612c3242ee95
        );
        assertEq(account.REVOKE_DEVICE_TYPEHASH(), 0xc3323bbac7e11d8f4946087779bc3f90a673ec602cab93c377d33aa1f5648f8e);
        assertEq(account.SET_RECOVERY_TYPEHASH(), 0xb6ec3e464c0650bda67f76c0b0d7d72afcd16d403c610d18cd7793118c1a6ed4);
        assertEq(
            account.INITIATE_RECOVERY_TYPEHASH(), 0x51e31a9ec813ed442f9a9de0ee7277f8c8e42ec3e189a8058bf7acb5da81aae7
        );
        assertEq(account.CANCEL_RECOVERY_TYPEHASH(), 0x60cd184d185c26b90deb8d0cbc00bb5573decf2eb83a18b2216a0f7d986d90ef);
        assertEq(account.CHANGE_RECOVERY_TYPEHASH(), 0x58eec495246ff7a500a54571691c10a40c638b112cc942c62b5aea7cca96d8d1);
        assertEq(
            registry.REGISTRATION_COMMITMENT_TYPEHASH(),
            0x916bdb07dc63f8f944e630d491d633db4e254b88c225dda462fbae8afc34e6e4
        );
        assertEq(registry.REGISTER_SHOT_TYPEHASH(), 0xc356ba3244a346558a5821261a4eccfb38382e0f90a60dc903003a671d5e828c);
        assertEq(
            registry.APPEND_CHECKPOINT_TYPEHASH(), 0x4ada9482c2ee717b1b8faa0707d2096906a4cc7d3e9ab28cf94f2b8d220e22f5
        );
        assertEq(registry.TRANSFER_SHOT_TYPEHASH(), 0x1b48fe9103fb5a4d3c6d61b8a9c98fada30123086d138b1c3c4407fa467c6d22);
    }

    function testEachContractHasItsOwnExactDomain() public {
        BuilderAccount account = newAccount(KEY1_X, KEY1_Y);
        ShotRegistry registry = new ShotRegistry();

        assertEq(account.domainSeparator(), _domain("TOHSENO BuilderAccount", address(account)));
        assertEq(registry.domainSeparator(), _domain("TOHSENO ShotRegistry", "2", address(registry)));
    }

    function testRegistryHashFunctionsUseTheDocumentedFieldOrder() public {
        ShotRegistry registry = new ShotRegistry();
        ShotRegistry.RegisterShotAction memory create = ShotRegistry.RegisterShotAction({
            shotId: keccak256("shot"),
            controller: address(0x1234),
            head: keccak256("head"),
            salt: keccak256("salt"),
            nonce: 7,
            deadline: 2_000_000_000
        });
        bytes32 createStructHash = keccak256(
            abi.encode(
                registry.REGISTER_SHOT_TYPEHASH(),
                create.shotId,
                create.controller,
                create.head,
                create.salt,
                create.nonce,
                create.deadline
            )
        );
        assertEq(registry.hashRegisterShot(create), _typedDigest(registry.domainSeparator(), createStructHash));

        ShotRegistry.AppendCheckpointAction memory append = ShotRegistry.AppendCheckpointAction({
            shotId: create.shotId,
            previousHead: create.head,
            newHead: keccak256("next-head"),
            checkpointSequence: 2,
            nonce: 1,
            deadline: create.deadline
        });
        bytes32 appendStructHash = keccak256(
            abi.encode(
                registry.APPEND_CHECKPOINT_TYPEHASH(),
                append.shotId,
                append.previousHead,
                append.newHead,
                append.checkpointSequence,
                append.nonce,
                append.deadline
            )
        );
        assertEq(registry.hashAppendCheckpoint(append), _typedDigest(registry.domainSeparator(), appendStructHash));

        ShotRegistry.TransferShotAction memory transfer = ShotRegistry.TransferShotAction({
            shotId: create.shotId,
            currentController: create.controller,
            newController: address(0x5678),
            currentHead: append.newHead,
            checkpointSequence: 2,
            nonce: 2,
            deadline: create.deadline
        });
        bytes32 transferStructHash = keccak256(
            abi.encode(
                registry.TRANSFER_SHOT_TYPEHASH(),
                transfer.shotId,
                transfer.currentController,
                transfer.newController,
                transfer.currentHead,
                transfer.checkpointSequence,
                transfer.nonce,
                transfer.deadline
            )
        );
        assertEq(registry.hashTransferShot(transfer), _typedDigest(registry.domainSeparator(), transferStructHash));
    }

    function testRecoveryHashFunctionsUseTheDocumentedFieldOrder() public {
        BuilderAccount account = newAccount(KEY1_X, KEY1_Y);
        address nextRecovery = address(0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB);
        uint64 deadline = 2_000_000_000;
        bytes32 replacementKeyId = account.deviceKeyId(KEY2_X, KEY2_Y);

        bytes32 initiateStructHash = keccak256(
            abi.encode(
                account.INITIATE_RECOVERY_TYPEHASH(),
                address(account),
                address(0),
                nextRecovery,
                replacementKeyId,
                KEY2_X,
                KEY2_Y,
                uint64(3),
                deadline
            )
        );
        assertEq(
            account.hashInitiateRecovery(KEY2_X, KEY2_Y, nextRecovery, 3, deadline),
            _typedDigest(account.domainSeparator(), initiateStructHash)
        );

        bytes32 changeStructHash = keccak256(
            abi.encode(
                account.CHANGE_RECOVERY_TYPEHASH(), address(account), address(0), nextRecovery, uint64(7), deadline
            )
        );
        assertEq(
            account.hashChangeRecovery(nextRecovery, 7, deadline),
            _typedDigest(account.domainSeparator(), changeStructHash)
        );

        bytes32 recoveryId = keccak256("recovery");
        bytes32 cancelStructHash =
            keccak256(abi.encode(account.CANCEL_RECOVERY_TYPEHASH(), address(account), recoveryId, uint64(8), deadline));
        assertEq(
            account.hashCancelRecovery(recoveryId, 8, deadline),
            _typedDigest(account.domainSeparator(), cancelStructHash)
        );
    }

    function _domain(string memory name, address verifyingContract) private view returns (bytes32) {
        return _domain(name, "1", verifyingContract);
    }

    function _domain(string memory name, string memory version, address verifyingContract)
        private
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                DOMAIN_TYPEHASH, keccak256(bytes(name)), keccak256(bytes(version)), block.chainid, verifyingContract
            )
        );
    }

    function _typedDigest(bytes32 domainSeparator, bytes32 structHash) private pure returns (bytes32) {
        return keccak256(abi.encodePacked(hex"1901", domainSeparator, structHash));
    }
}
