#!/bin/sh
set -eu

version="v0.6.0"
repository="https://github.com/jpfraneto/tohseno"

if [ "$(uname -s)" != "Darwin" ]; then
  printf '%s\n' "Run this installer on a Mac."
  exit 1
fi

case "$(uname -m)" in
  arm64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *)
    printf '%s\n' "Use an Apple silicon or Intel Mac."
    exit 1
    ;;
esac

install_directory="$HOME/.tohseno/bin"
install_temporary="$(mktemp -d "${TMPDIR:-/tmp}/tohseno-install.XXXXXX")"
trap 'rm -rf "$install_temporary"' EXIT HUP INT TERM
mkdir -p "$install_directory"

binary_name="tohseno-$target"
curl -fsSL "$repository/releases/download/$version/$binary_name" -o "$install_temporary/tohseno"
chmod 0755 "$install_temporary/tohseno"
mv "$install_temporary/tohseno" "$install_directory/tohseno"

case "${SHELL:-}" in
  */zsh) shell_file="$HOME/.zshrc" ;;
  */bash) shell_file="$HOME/.bashrc" ;;
  *) shell_file="$HOME/.profile" ;;
esac

path_line='export PATH="$HOME/.tohseno/bin:$PATH"'
touch "$shell_file"
if ! grep -Fqx "$path_line" "$shell_file"; then
  printf '\n%s\n' "$path_line" >> "$shell_file"
  printf 'Add TOHSENO to this shell now with: source %s\n' "$shell_file"
else
  printf '%s\n' "TOHSENO is already on your saved PATH."
fi

"$install_directory/tohseno" --version
"$install_directory/tohseno" doctor --background >/dev/null 2>&1 &
