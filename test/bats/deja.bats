
setup() {
    load 'test_helper'
    deja_setup
}

@test "(no args)" {
  deja
  assert_handled_failure
}

@test "--help" {
  deja --help
  assert_success
}

@test "--version" {
  deja --version
  assert_success
}

@test "run" {
  deja run -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run -- uuidgen
  assert_success_with_uuid_matching $first_output "returns previous result"
}

@test "run --watch-path" {
  folder=$(folder_fixture folder)
  other_folder=$(folder_fixture other_folder)

  deja run --watch-path $folder -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run --watch-path $folder -- uuidgen
  assert_success_with_uuid_matching $first_output "returns previous result"

  deja run --watch-path $other_folder -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result for different path"

  touch $folder/file
  deja run --watch-path $folder -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result when watched path changes"
}

@test "run --watch-scope" {
  deja run --watch-scope a -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run --watch-scope a -- uuidgen
  assert_success_with_uuid_matching $first_output "returns previous result"

  deja run --watch-scope b -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result for different scope"

  deja run --watch-scope a --watch-scope b -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result for with extra scope"

  deja run --watch-scope a -- uuidgen
  assert_success_with_uuid_matching $first_output "still returns result when called with original scope"
}

@test "run --watch-env" {
  ENV_A=1 deja run --watch-env ENV_A -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  ENV_A=1 deja run --watch-env ENV_A -- uuidgen
  assert_success_with_uuid_matching $first_output "returns previous result"

  ENV_A=1 ENV_B=2 deja run --watch-env ENV_A -- uuidgen
  assert_success_with_uuid_matching $first_output "returns previous result if other env set"

  ENV_A=2 deja run --watch-env ENV_A -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result with different env value"

  deja run --watch-env ENV_A -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result when env not set"
}

@test "run --look-back" {
  deja run -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  sleep 1

  deja run --look-back 5s -- uuidgen
  assert_success_with_uuid_matching $first_output "returns previous result looking back 5s"

  deja run --look-back 1s -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result if cached result is too stale"

  fresh_output=$output

  deja run --look-back 1s -- uuidgen
  assert_success_with_uuid_matching $fresh_output "new result is now cached for future calls"

  deja run --look-back 1s -- uuidgen
  assert_success_with_uuid_matching $fresh_output "new result is also returned when no look back specified"
}

@test "run --cache-for" {
  deja run --cache-for 1s -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run -- uuidgen
  assert_success_with_uuid_matching $first_output "returns result when called within cache-for period"

  sleep 1

  deja run -- uuidgen
  assert_success_with_uuid_not_matching $first_output "returns fresh result if cached result has expired"
}

@test "run --exclude-pwd" {
  folder=$(folder_fixture folder)

  deja run -- uuidgen
  assert_success_with_uuid "runs command and returns result"

  output_without_flag=$output

  deja run --debug --exclude-pwd -- uuidgen
  assert_success_with_uuid_not_matching $output_without_flag "generates different result when --exclude-pwd flag is set"

  output_with_flag=$output

  deja run --debug --exclude-pwd -- uuidgen
  assert_success_with_uuid_matching $output_with_flag "returns previous result when --exclude-pwd flag is set"

  cd $folder

  deja run -- uuidgen
  assert_success_with_uuid_not_matching $output_without_flag "returns different result when called without flag from different folder"
  assert_not_equal "$output" "$output_with_flag" "doesn't return output generated with flag when called without flag"


  deja run --debug --exclude-pwd -- uuidgen
  assert_success_with_uuid_matching $output_with_flag "returns previous result from when called with flag from different folder"
}

@test "run (error: command not found)" {
  deja run -- unknown
  assert_handled_failure "fails when unknown command"
  assert_equal "$stderr" "deja: command not found: unknown"
}

@test "run (error: permission denied to run command)" {
  deja run -- ./README.md
  assert_handled_failure "fails when unknown command"
  assert_equal "$stderr" "deja: permission denied running command: ./README.md"
}

@test "run (error: cache location inaccessible)" {
  deja run --cache /missing/folder -- uuidgen
  assert_handled_failure "fails when unknown command"
  assert_equal "$stderr" "deja: unable to write to cache /missing/folder"
}

@test "run --look-back (error: invalid duration)" {
  deja run --look-back 1xyz -- uuidgen
  assert_handled_failure "fails when duration can't be parsed"
  assert_equal "$stderr" "deja: invalid duration '1xyz', use values like 15s, 30m, 3h, 4d etc"
}

@test "run --cache-for (error: invalid duration)" {
  deja run --cache-for 1xyz -- uuidgen
  assert_handled_failure "fails when duration can't be parsed"
  assert_equal "$stderr" "deja: invalid duration '1xyz', use values like 15s, 30m, 3h, 4d etc"
}

@test "read" {
  deja read -- uuidgen
  assert_handled_failure "fails when no result cached"

  deja run -- uuidgen
  first_output=$output

  deja read -- uuidgen
  assert_success_with_uuid_matching $first_output "returns cached result"
}

@test "force" {
  deja run -- uuidgen

  first_output=$output

  deja force -- uuidgen
  assert_success_with_uuid_not_matching $first_output "forces new result"

  forced_output=$output

  deja run -- uuidgen
  assert_success_with_uuid_matching $forced_output "forced result now cached"
}

@test "remove" {
  deja run -- uuidgen

  first_output=$output

  deja remove -- uuidgen
  deja run -- uuidgen

  assert_success_with_uuid_not_matching $first_output "removing result forces new result on next call"

  deja remove -- uuidgen
  assert_success

  deja test -- uuidgen
  assert_handled_failure "removing result removes it from cache"

  deja remove -- uuidgen
  assert_handled_failure "removing result that doesn't exist fails"
}

@test "test" {
  deja test -- uuidgen
  assert_handled_failure "fails when no result cached"

  deja run -- uuidgen

  deja test -- uuidgen
  assert_success "succeeds now result cached"

  deja remove -- uuidgen

  deja test -- uuidgen
  assert_handled_failure "fails when result removed"
}

@test "explain" {
  deja explain -- uuidgen
  assert_success
}
