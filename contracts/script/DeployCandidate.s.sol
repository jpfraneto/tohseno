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

    bytes32 public constant FACTORY_SALT = 0x2d32554fb15a503d75b83ce5d8d53a828c7420f84de1ee1c80af7ee773521800;
    bytes32 public constant REGISTRY_SALT = 0xbcf41492063a04daa488cd46b8e0e62d6cca2e1da41f58f5bfae84ad42ab6a0f;
    bytes32 public constant RELATIONS_SALT = 0xecb33008d6d462cb873510b5bd93291242ca14f4156a2fae7d9f04dd9a956c25;

    address public constant FACTORY = 0xb802F0ef1595734f2529f602F2473d829d6aaFaF;
    address public constant REGISTRY = 0x5DAf4fA6c285AFb4B19978AD56A3892e7676Cc68;
    address public constant RELATIONS = 0xb7Dc8acfBfC5D93146e4e88D12e5223a5E6A3b83;

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
