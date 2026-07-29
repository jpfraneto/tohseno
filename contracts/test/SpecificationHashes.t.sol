// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

import {ProtocolTestBase} from "./ProtocolTestBase.sol";
import {BuilderAccount} from "../src/BuilderAccount.sol";
import {ShotRegistry} from "../src/ShotRegistry.sol";
import {ShotRelations} from "../src/ShotRelations.sol";

contract SpecificationHashesTest is ProtocolTestBase {
    bytes32 private constant DOMAIN_TYPEHASH = 0x8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f;

    function testTypeHashesAreTheDocumentedProtocolConstants() public {
        BuilderAccount account = newAccount(KEY1_X, KEY1_Y);
        ShotRegistry registry = new ShotRegistry();
        ShotRelations relations = new ShotRelations(address(registry));

        assertEq(
            account.AUTHORIZE_DEVICE_TYPEHASH(), 0xcdb0126f85b19b28642fc350e8d771c41afd39854da572f865a3612c3242ee95
        );
        assertEq(account.REVOKE_DEVICE_TYPEHASH(), 0xc3323bbac7e11d8f4946087779bc3f90a673ec602cab93c377d33aa1f5648f8e);
        assertEq(account.SET_RECOVERY_TYPEHASH(), 0xb6ec3e464c0650bda67f76c0b0d7d72afcd16d403c610d18cd7793118c1a6ed4);
        assertEq(account.RECOVER_ACCOUNT_TYPEHASH(), 0x539858b6492297e81f07fffbe9f57c1363d0529cf5783087d05848f72fedb820);
        assertEq(registry.CREATE_SHOT_TYPEHASH(), 0xe627bc9302992c61fc4043b351fb7d7551f9ed0e0753a1e76a0e68e7a9a60b99);
        assertEq(
            registry.APPEND_EVOLUTION_TYPEHASH(), 0x3a0d9d9dfaedfea172f8ba24e22ce2e86abf77a208168a5246f7a7be2d72de67
        );
        assertEq(registry.TRANSFER_SHOT_TYPEHASH(), 0x0de266e673064af8f761f1bf3366a2565f963d431128044ec828ab11fcff4e62);
        assertEq(
            registry.SET_PUBLIC_STATE_TYPEHASH(), 0x1ab3043484eb2409f5e939f9f5666eac599cffe910808e5711554613bc513c2c
        );
        assertEq(relations.CLAIM_HANDLE_TYPEHASH(), 0x01bbf31631b6914cf984c8f743c927970ade109f7195087d5670a72ae645ac0d);
        assertEq(
            relations.RELEASE_HANDLE_TYPEHASH(), 0x1671629a28ceca947ec5843ce55671eda943cfcc82e96017424e06ce90c17e74
        );
        assertEq(
            relations.ASSOCIATE_APPCOIN_TYPEHASH(), 0x7872c921a764a03a974ddf09ac8a0086034006a1a64534f83111087a00ff71d7
        );
        assertEq(
            relations.REMOVE_APPCOIN_TYPEHASH(), 0xd28f6beda3c41ce6f132f41a0800be466a97d65ab19b16c69f5a9202fa85ef3e
        );
        assertEq(
            relations.ATTEST_APP_STORE_TYPEHASH(), 0xb6f951e532ceef1e89f76812699e4039dac302f458a23fa83e69dffa475d1e96
        );
    }

    function testEachContractHasItsOwnExactDomain() public {
        BuilderAccount account = newAccount(KEY1_X, KEY1_Y);
        ShotRegistry registry = new ShotRegistry();
        ShotRelations relations = new ShotRelations(address(registry));

        assertEq(account.domainSeparator(), _domain("TOHSENO BuilderAccount", address(account)));
        assertEq(registry.domainSeparator(), _domain("TOHSENO ShotRegistry", address(registry)));
        assertEq(relations.domainSeparator(), _domain("TOHSENO ShotRelations", address(relations)));
    }

    function _domain(string memory name, address verifyingContract) private view returns (bytes32) {
        return keccak256(
            abi.encode(DOMAIN_TYPEHASH, keccak256(bytes(name)), keccak256(bytes("1")), block.chainid, verifyingContract)
        );
    }
}
