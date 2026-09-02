#!/bin/sh

# Sourced by macOS verification scripts. The caller owns one already-created,
# private temporary root and removes it after this exact Keychain is deleted.

verification_keychain_path=""
verification_keychain_service=""
verification_service_label=""

create_tohseno_verification_keychain() {
  verification_root="$1"
  verification_suffix="$2"
  case "$verification_root" in
    /*) ;;
    *) return 1 ;;
  esac
  [ -d "$verification_root" ] && [ ! -L "$verification_root" ] || return 1
  case "$verification_suffix" in
    ""|*[!a-z0-9.-]*|.*|*.) return 1 ;;
  esac

  verification_keychain_path="$verification_root/tohseno-verification.keychain-db"
  [ ! -e "$verification_keychain_path" ] && [ ! -L "$verification_keychain_path" ] || return 1
  /usr/bin/security create-keychain -p '' "$verification_keychain_path" || return 1
  if [ ! -f "$verification_keychain_path" ] || [ -L "$verification_keychain_path" ] ||
    ! /usr/bin/security unlock-keychain -p '' "$verification_keychain_path"; then
    /usr/bin/security delete-keychain "$verification_keychain_path" >/dev/null 2>&1 || true
    verification_keychain_path=""
    return 1
  fi
  verification_keychain_service="com.tohseno.workspace-service.verification.$verification_suffix"
  verification_service_label="com.tohseno.workspace-service.verification.$verification_suffix"
}

delete_tohseno_verification_keychain() {
  verification_root="$1"
  [ -n "$verification_keychain_path" ] || return 0
  case "$verification_keychain_path" in
    "$verification_root"/tohseno-verification.keychain-db) ;;
    *) return 1 ;;
  esac
  if [ -L "$verification_keychain_path" ]; then
    return 1
  fi
  if [ -e "$verification_keychain_path" ]; then
    /usr/bin/security delete-keychain "$verification_keychain_path" || return 1
  fi
  verification_keychain_path=""
  verification_keychain_service=""
  verification_service_label=""
}
