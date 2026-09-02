#include "CTohsenoVerificationKeychain.h"

#if TARGET_OS_OSX
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"

OSStatus TohsenoVerificationKeychainOpen(
    const char *path_name,
    SecKeychainRef *keychain
) {
    return SecKeychainOpen(path_name, keychain);
}

OSStatus TohsenoVerificationKeychainUnlock(
    SecKeychainRef keychain,
    UInt32 password_length,
    const void *password,
    Boolean use_password
) {
    return SecKeychainUnlock(
        keychain,
        password_length,
        password,
        use_password
    );
}

#pragma clang diagnostic pop
#endif
