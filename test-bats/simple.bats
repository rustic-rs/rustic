# Be extra careful to not mess with a user repository
unset RUSTIC_REPOSITORY
unset RUSTIC_PASSWORD
unset RUSTIC_PASSWORD_FILE
unset RUSTIC_PASSWORD_COMMAND

export REPO_DIR="${BATS_TMPDIR%/}/repo"
export RESTORE_DIR="${BATS_TMPDIR%/}/restore"
export RUSTIC_REPOSITORY=$REPO_DIR
export RUSTIC_PASSWORD=test

setup() {
  # get the containing directory of this file
  # use $BATS_TEST_FILENAME instead of ${BASH_SOURCE[0]} or $0,
  # as those will point to the bats executable's location or the preprocessed file respectively
  DIR="$( cd "$( dirname "$BATS_TEST_FILENAME" )" >/dev/null 2>&1 && pwd )"
  BASEDIR="$( cd $DIR/..  >/dev/null 2>&1 && pwd )"
  load "$DIR/bats-support/load"
  load "$DIR/bats-assert/load"
  # make executables in src/ visible to PATH
  RUSTIC="$DIR/../target/release/rustic"
  echo $RUSTIC
  rm -rf "$REPO_DIR"
  run $RUSTIC init
  assert_success
  assert_output -p "successfully created"
}

teardown () {
  chmod -R 700 "$REPO_DIR"
  rm -rf "$REPO_DIR"
}

@test "backup and check" {
   run $RUSTIC backup $BASEDIR/src
   assert_success
   assert_output -p "successfully saved"

   run $RUSTIC snapshots
   assert_success
   assert_output -p "1 snapshot(s)"

   run $RUSTIC backup $BASEDIR/src
   assert_success
   assert_output -p "Added to the repo: 0 B"
   assert_output -p "successfully saved"

   run $RUSTIC snapshots
   assert_success
   assert_output -p "2 snapshot(s)"

   run $RUSTIC check --read-data
   assert_success
   refute_output -p "ERROR"
   refute_output -p "WARN"
}

@test "backup and restore" {
   run $RUSTIC backup $BASEDIR/src
   assert_success
   assert_output -p "successfully saved"

   run $RUSTIC restore latest:$BASEDIR/src $RESTORE_DIR
   assert_success
   assert_output -p "restore done"

   run diff -r $BASEDIR/src $RESTORE_DIR
   assert_output ""
}

