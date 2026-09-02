#ifndef C_TOHSENO_VERIFICATION_KEYCHAIN_H
#define C_TOHSENO_VERIFICATION_KEYCHAIN_H

#include <Security/Security.h>
#include <TargetConditionals.h>

#if TARGET_OS_OSX
OSStatus TohsenoVerificationKeychainOpen(
    const char * _Nonnull path_name,
    SecKeychainRef _Nullable * _Nonnull keychain
);

OSStatus TohsenoVerificationKeychainUnlock(
    SecKeychainRef _Nullable keychain,
    UInt32 password_length,
    const void * _Nullable password,
    Boolean use_password
);
#endif

#endif
