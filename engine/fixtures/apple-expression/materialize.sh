#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
  printf '%s\n' \
    "usage: engine/fixtures/apple-expression/materialize.sh DESTINATION APP_NAME BUNDLE_ID" >&2
  exit 64
fi

destination=$1
app_name=$2
bundle_id=$3

case "$app_name" in
  ""|*[!A-Za-z0-9-]*|-*|*-|*--*)
    printf '%s\n' "fixture app name is not a TOHSENO-safe component: $app_name" >&2
    exit 64
    ;;
esac
case "$bundle_id" in
  ""|*[!A-Za-z0-9.-]*|.*|*.|*..*)
    printf '%s\n' "fixture bundle identifier is unsafe: $bundle_id" >&2
    exit 64
    ;;
esac

if [ -L "$destination" ] || [ ! -d "$destination" ]; then
  printf '%s\n' "fixture destination must be an existing non-symlink directory" >&2
  exit 66
fi

fixture_directory=$(
  CDPATH= cd -- "$(dirname -- "$0")" >/dev/null 2>&1
  pwd -P
)
repository_root=$(
  CDPATH= cd -- "$fixture_directory/../../.." >/dev/null 2>&1
  pwd -P
)
project_destination="$destination/$app_name.xcodeproj"

for path in \
  "$destination/TemplateApp.swift" \
  "$destination/TohsenoFascia" \
  "$destination/TOHSENO" \
  "$destination/MEMORY.md" \
  "$destination/WORLD.md" \
  "$project_destination"
do
  if [ -e "$path" ] || [ -L "$path" ]; then
    printf '%s\n' "fixture refuses to overwrite: $path" >&2
    exit 73
  fi
done

mkdir "$project_destination"
mkdir "$destination/TohsenoFascia"
mkdir "$destination/TOHSENO"

sed \
  -e "s|__APP_NAME__|$app_name|g" \
  -e "s|__BUNDLE_ID__|$bundle_id|g" \
  "$fixture_directory/Template.xcodeproj/project.pbxproj" \
  >"$project_destination/project.pbxproj"

install -m 0644 "$fixture_directory/TemplateApp.swift" "$destination/TemplateApp.swift"
install -m 0644 "$fixture_directory/MEMORY.md" "$destination/MEMORY.md"
install -m 0644 "$fixture_directory/WORLD.md" "$destination/WORLD.md"

for source in \
  InstallationIdentity.swift \
  ContinuityEnvelope.swift \
  LocalPersistence.swift \
  Provenance.swift \
  TohsenoMetadata.swift
do
  install -m 0644 \
    "$repository_root/fascia/apple/swift/$source" \
    "$destination/TohsenoFascia/$source"
done

printf '{}\n' >"$destination/TOHSENO/fascia.json"
printf '{}\n' >"$destination/TOHSENO/embedded-provenance.json"

printf '%s\n' "materialized Apple expression fixture: $destination"
