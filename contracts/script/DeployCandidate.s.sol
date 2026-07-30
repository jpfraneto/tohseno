// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.30;

interface Vm {
    function getCode(string calldata artifactPath) external view returns (bytes memory creationCode);
    function startBroadcast() external;
    function stopBroadcast() external;
}

/// @notice Deploys or verifies the fixed GENESIS candidate through the Arachnid CREATE2 deployer.
/// @dev The surrounding shell procedure performs chain, signer, artifact, and confirmation checks.
contract DeployCandidate {
    Vm private constant VM = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    address public constant DETERMINISTIC_DEPLOYER = 0x4e59b44847b379578588920cA78FbF26c0B4956C;
    bytes32 public constant DETERMINISTIC_DEPLOYER_CODE_HASH =
        0x2fa86add0aed31f33a762c9d88e807c475bd51d0f52bd0955754b2608f7e4989;

    bytes32 public constant FACTORY_SALT = 0xe0fd0e28bcdb28bfdfa44c2bba736c6206a798abf890d7d5690e5b77610c603a;
    bytes32 public constant REGISTRY_SALT = 0x28355a607bb3452ad437f71bc1b14e43f270e721b2bfbf028a867711aa473af1;
    bytes32 public constant RELATIONS_SALT = 0xdb5c183795d37085c73de55b04fb086beaa54ab4bde52b3c573952af82200ab3;

    address public constant FACTORY = 0x9A48926c82FE766fe599116dFc7111Ba6F7171DD;
    address public constant REGISTRY = 0x02D2A9ED5ba8843b82b4e5976C686DCE4AF3bA5e;
    address public constant RELATIONS = 0x75ABff418c4Cad3c4bD56f467cC2737237dd6eA5;

    bytes32 public constant FACTORY_RUNTIME_CODE_HASH =
        0x1f44f9fa643277e05f5a9d1f6a05b4cee9264c261a423021c5e0c7f5da3b312a;
    bytes32 public constant REGISTRY_RUNTIME_CODE_HASH =
        0xac64e4933d88d40c18af598f7ebf7bc8f7b829e1a61acb8e380d4ac670f31478;
    bytes32 public constant RELATIONS_RUNTIME_CODE_HASH =
        0x909ba083f6b186b08f80d5ea465878f7a0c909f1c65b11d2ed8ca11a40669de5;

    error WrongChain(uint256 observed);
    error DeterministicDeployerMismatch(bytes32 observed);
    error ExistingCodeMismatch(address deployment, bytes32 expected, bytes32 observed);
    error DeploymentFailed(address expected);
    error InvalidDeploymentReturn(address expected);
    error WrongDeploymentAddress(address expected, address observed);

    event CandidateContract(
        bytes32 indexed label, address indexed deployment, bytes32 runtimeCodeHash, bool alreadyExisted
    );

    function run() external {
        if (block.chainid != 4663) revert WrongChain(block.chainid);
        bytes32 deployerHash = DETERMINISTIC_DEPLOYER.codehash;
        if (deployerHash != DETERMINISTIC_DEPLOYER_CODE_HASH) {
            revert DeterministicDeployerMismatch(deployerHash);
        }

        bytes memory factoryInitCode = VM.getCode("BuilderAccountFactory.sol:BuilderAccountFactory");
        bytes memory registryInitCode = VM.getCode("ShotRegistry.sol:ShotRegistry");
        bytes memory relationsInitCode =
            abi.encodePacked(VM.getCode("ShotRelations.sol:ShotRelations"), abi.encode(REGISTRY));

        VM.startBroadcast();
        _deployOrVerify(
            keccak256("BuilderAccountFactory"), FACTORY, FACTORY_SALT, factoryInitCode, FACTORY_RUNTIME_CODE_HASH
        );
        _deployOrVerify(
            keccak256("ShotRegistry"), REGISTRY, REGISTRY_SALT, registryInitCode, REGISTRY_RUNTIME_CODE_HASH
        );
        _deployOrVerify(
            keccak256("ShotRelations"), RELATIONS, RELATIONS_SALT, relationsInitCode, RELATIONS_RUNTIME_CODE_HASH
        );
        VM.stopBroadcast();
    }

    function _deployOrVerify(
        bytes32 label,
        address expected,
        bytes32 salt,
        bytes memory initCode,
        bytes32 expectedRuntimeHash
    ) private {
        bytes32 observedHash = expected.codehash;
        bool alreadyExisted = expected.code.length != 0;
        if (!alreadyExisted) {
            (bool success, bytes memory output) = DETERMINISTIC_DEPLOYER.call(abi.encodePacked(salt, initCode));
            if (!success) revert DeploymentFailed(expected);
            if (output.length != 20) revert InvalidDeploymentReturn(expected);

            address observed;
            assembly ("memory-safe") {
                observed := shr(96, mload(add(output, 32)))
            }
            if (observed != expected) revert WrongDeploymentAddress(expected, observed);
            observedHash = expected.codehash;
        }
        if (observedHash != expectedRuntimeHash) {
            revert ExistingCodeMismatch(expected, expectedRuntimeHash, observedHash);
        }
        emit CandidateContract(label, expected, observedHash, alreadyExisted);
    }
}
