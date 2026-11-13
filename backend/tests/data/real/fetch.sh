#!/bin/sh
# Download the public SO-ARM100 models (TheRobotStudio, Apache-2.0) that the
# ignored real-model tests of cad_import_test.rs import, pinned by commit and
# SHA-256, into this directory (the .step files are not committed).
set -eu
cd "$(dirname "$0")"
base=https://raw.githubusercontent.com/TheRobotStudio/SO-ARM100/a758567c3978dfeefe282ede0500085a48fe8f78/STEP

# fetch <file> <path under STEP/> <SHA-256>
fetch() {
  if [ -f "$1" ] && echo "$3  $1" | sha256sum -c --status; then return; fi
  curl -fsSL -o "$1.part" "$base/$2"
  echo "$3  $1.part" | sha256sum -c --quiet
  mv "$1.part" "$1"
}
fetch upper_arm.step SO101/Upper_arm_SO101.step efa19a6dd2ccb459248500c76629cfa840630e7e15d9e146394d31da1525dd61
fetch so100.step SO100/Follower_Specific/SO_5DOF_ARM100_Assembly.step 6dda8ce9858b2a460082eb192678ba4dcfc062d8bbacc160111bf2019b8c6658
fetch so101.step "SO101/SO101%20Assembly.step" f754be35708382b05ce65768ae33c612f91dca11ad19d14807dc637eb944ba3d
