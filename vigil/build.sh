#!/bin/bash
# This build file will build using custom remote dependency forks unless
# specified local options are available, in which case the local is used.

# Specify the local overrides for custom remote dependencies.
OVERRIDES="
libp2p;../../rust-libp2p/libp2p/;https://github.com/unattended-backpack/rust-libp2p.git
"

# Set a build flag, cleared however this script exits.
function reset {
  export BUILD_SCRIPT_USED=0
}
trap reset EXIT
export BUILD_SCRIPT_USED=1

# Ensure the .cargo directory exists.
mkdir -p ".cargo"

# Generate a patch file for any local overrides.
rm -f ".cargo/config.toml"
CONFIG_TOML_CONTENT=""
while IFS= read -r MAPPING; do
  [ -z "$MAPPING" ] && continue

  # Split the mapping into crate name, local path, and remote Git URL
  IFS=';' read -ra DEP_INFO <<< "$MAPPING"
  CRATE_NAME="${DEP_INFO[0]}"
  LOCAL_PATH="${DEP_INFO[1]}"
  REMOTE_GIT_URL="${DEP_INFO[2]}"

  # The local dependency is present; use it.
  if [ -d "$LOCAL_PATH" ]; then
    CONFIG_TOML_CONTENT+="
[patch.\"$REMOTE_GIT_URL\"]
$CRATE_NAME = { path = \"$LOCAL_PATH\" }
"
    echo "-> using local $CRATE_NAME at $LOCAL_PATH ..."
  
  # The local dependency is not present; use the updated remote.
  else
    echo "-> local $CRATE_NAME not found: using updated remote ..."
    cargo clean -p $CRATE_NAME
    Ecargo update -p $CRATE_NAME
  fi
done <<< "$OVERRIDES"

# Write the new patch file.
if [ -n "$CONFIG_TOML_CONTENT" ]; then
  echo "$CONFIG_TOML_CONTENT" > ".cargo/config.toml"
fi

# Finally, perform the build.
cargo build
